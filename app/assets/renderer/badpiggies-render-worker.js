const backend = "worker";
const runtimeThreadCount = 1;
const fallbackReason = "";
const rendererVersion = "20260718-pointer-world-3";

const wasmReady = (async () => {
    const runtime = await import(
        `./pkg/badpiggies_editor_renderer.js?v=${rendererVersion}`
    );
    await runtime.default({
        module_or_path: `./pkg/badpiggies_editor_renderer_bg.wasm?v=${rendererVersion}`,
    });
    return runtime;
})();

let handle = null;
let running = false;
let animationFrame = 0;
let timer = 0;
let framePending = false;
let lastCursor = "default";
let reportFrameStats = false;

globalThis.bpRendererEvent = (event) => {
    self.postMessage({ type: "renderer_event", event });
};

function errorMessage(error) {
    return error instanceof Error ? error.message : String(error);
}

function stopFrameLoop() {
    running = false;
    if (animationFrame && typeof self.cancelAnimationFrame === "function") {
        self.cancelAnimationFrame(animationFrame);
    }
    if (timer) self.clearTimeout(timer);
    animationFrame = 0;
    timer = 0;
    framePending = false;
}

function scheduleFrame() {
    if (!running || framePending) return;
    framePending = true;
    if (typeof self.requestAnimationFrame === "function") {
        animationFrame = self.requestAnimationFrame(renderFrame);
    } else {
        timer = self.setTimeout(() => renderFrame(performance.now()), 16);
    }
}

function renderFrame(timestamp) {
    animationFrame = 0;
    timer = 0;
    framePending = false;
    if (!running || !handle) return;
    try {
        const cursor = handle.frame(timestamp);
        if (cursor !== lastCursor) {
            lastCursor = cursor;
            self.postMessage({ type: "cursor", cursor });
        }
        if (reportFrameStats) {
            self.postMessage({ type: "frame_stats", stats: handle.frame_stats() });
        }
        if (handle.needs_repaint()) scheduleFrame();
    } catch (error) {
        stopFrameLoop();
        self.postMessage({ type: "error", message: errorMessage(error) });
    }
}

async function startRenderer(message) {
    const runtime = await wasmReady;
    handle = new runtime.RendererHandle();
    await handle.start_offscreen(
        message.canvas,
        message.assetRoot,
        message.width,
        message.height,
    );
    reportFrameStats = Boolean(message.reportStats);
    running = true;
    scheduleFrame();
    self.postMessage({
        type: "ready",
        backend,
        threadCount: runtimeThreadCount,
        fontBackend: handle.font_backend(),
        fallbackReason,
    });
    self.setTimeout(() => {
        try {
            handle?.warm_up();
            self.postMessage({ type: "warmed" });
        } catch (error) {
            console.warn("Renderer asset warm-up failed.", error);
            self.postMessage({ type: "warmed", error: errorMessage(error) });
        }
    }, 0);
}

self.onmessage = async (event) => {
    const message = event.data ?? {};
    try {
        switch (message.type) {
            case "probe":
                self.postMessage({
                    type: "probe",
                    supported:
                        typeof OffscreenCanvas !== "undefined" &&
                        typeof self.navigator?.gpu !== "undefined",
                });
                break;
            case "init":
                await startRenderer(message);
                break;
            case "scene":
                handle?.set_scene(message.scene);
                scheduleFrame();
                break;
            case "view":
                handle?.set_view(message.view);
                scheduleFrame();
                break;
            case "command":
                handle?.command(message.name);
                scheduleFrame();
                break;
            case "resize":
                handle?.resize(message.width, message.height);
                scheduleFrame();
                break;
            case "pointer":
                handle?.pointer_event(
                    message.kind,
                    message.x,
                    message.y,
                    message.button,
                    message.detail,
                    message.alt,
                    message.ctrl,
                    message.shift,
                    message.command,
                );
                scheduleFrame();
                break;
            case "wheel":
                handle?.wheel(message.x, message.y);
                scheduleFrame();
                break;
            case "key":
                handle?.key(
                    message.key,
                    message.alt,
                    message.ctrl,
                    message.shift,
                    message.command,
                );
                scheduleFrame();
                break;
            case "touch_transform":
                handle?.touch_transform(message.zoom, message.dx, message.dy);
                scheduleFrame();
                break;
            case "destroy":
                stopFrameLoop();
                handle?.destroy();
                handle = null;
                self.close();
                break;
            default:
                break;
        }
    } catch (error) {
        self.postMessage({ type: "error", message: errorMessage(error) });
    }
};
