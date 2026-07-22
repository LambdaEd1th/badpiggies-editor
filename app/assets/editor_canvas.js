return (async function () {
    const RENDERER_VERSION = "20260723-mobile-object-gestures-1";
    const DIOXUS_ASSET_ROOT = __BP_ASSET_ROOT__;
    const REPORT_FRAME_STATS = new URLSearchParams(window.location.search).has("renderStats");
    window.bpEditorCanvas?.destroy();
    window.bpContraptionPreview?.destroy();

    const send = (message) => {
        try {
            dioxus.send(message);
        } catch (_) {
            // The evaluator may be shutting down with its component.
        }
    };

    let backend = null;
    let pendingScene = null;
    let pendingView = null;
    let lastSceneIdentity = null;
    let starting = false;
    let animationFrame = 0;
    let framePending = false;
    let removeInputListeners = null;
    let removeShortcutListener = null;
    let resizeObserver = null;
    let currentCanvas = null;
    let runtime = null;

    window.bpRendererEvent = (message) => send(message);

    function absoluteAsset(relative) {
        const root = new URL(DIOXUS_ASSET_ROOT, document.baseURI).href.replace(/\/$/, "");
        return relative ? `${root}/${relative}` : root;
    }

    function reportError(error) {
        const message = error instanceof Error ? error.message : String(error);
        console.error("Bad Piggies renderer:", error);
        send({ type: "error", message: `Renderer failed: ${message}` });
    }

    async function importRendererModule(moduleUrl) {
        try {
            const module = await import(moduleUrl);
            currentCanvas.dataset.rendererModule = "direct";
            return module;
        } catch (directError) {
            // WKWebView rejects ES-module imports from Dioxus' custom asset
            // protocol, although ordinary fetches from the same URL work.
            try {
                const response = await fetch(moduleUrl);
                if (!response.ok && response.status !== 0) {
                    throw new Error(`HTTP ${response.status} ${response.statusText}`);
                }
                const source = await response.text();
                if (!source.trim()) {
                    throw new Error("renderer module source is empty");
                }
                const blobUrl = URL.createObjectURL(new Blob(
                    [`${source}\n//# sourceURL=${moduleUrl}`],
                    { type: "text/javascript" },
                ));
                try {
                    const module = await import(blobUrl);
                    currentCanvas.dataset.rendererModule = "blob";
                    return module;
                } finally {
                    URL.revokeObjectURL(blobUrl);
                }
            } catch (fallbackError) {
                throw new Error(
                    `Unable to load renderer module (${moduleUrl}). ` +
                    `Direct import: ${String(directError)}. ` +
                    `Blob fallback: ${String(fallbackError)}`,
                );
            }
        }
    }

    function canvasSize(canvas) {
        const rect = canvas.getBoundingClientRect();
        return {
            width: Math.max(1, Math.round(rect.width)),
            height: Math.max(1, Math.round(rect.height)),
        };
    }

    function sceneView(scene) {
        const {
            document_key: _documentKey,
            revision: _revision,
            file_name: _fileName,
            level: _level,
            ...view
        } = scene;
        return view;
    }

    function post(message) {
        if (!backend) return;
        if (backend.kind === "worker") {
            backend.worker.postMessage(message);
            return;
        }
        const handle = backend.handle;
        switch (message.type) {
            case "scene":
                handle.set_scene(message.scene);
                break;
            case "view":
                handle.set_view(message.view);
                break;
            case "command":
                handle.command(message.name);
                break;
            case "resize":
                handle.resize(message.width, message.height);
                break;
            case "pointer":
                handle.pointer_event(
                    message.kind,
                    message.x,
                    message.y,
                    message.button,
                    message.detail,
                    message.alt,
                    message.ctrl,
                    message.shift,
                    message.command,
                    message.source || "mouse",
                );
                break;
            case "wheel":
                handle.wheel(message.x, message.y);
                break;
            case "key":
                handle.key(
                    message.key,
                    message.alt,
                    message.ctrl,
                    message.shift,
                    message.command,
                );
                break;
            case "touch_transform":
                handle.touch_transform(
                    message.zoom,
                    message.dx,
                    message.dy,
                    message.x,
                    message.y,
                );
                break;
            default:
                break;
        }
        requestMainFrame();
    }

    function deliverScene(scene, force = false) {
        if (!backend || !scene) return;
        const identity = `${scene.document_key}\u0000${scene.revision}`;
        if (force || identity !== lastSceneIdentity) {
            lastSceneIdentity = identity;
            post({ type: "scene", scene });
        } else {
            post({ type: "view", view: sceneView(scene) });
        }
    }

    function installRuntime(canvas) {
        runtime = {
            canvas,
            render(scene) {
                pendingScene = scene;
                pendingView = null;
                try {
                    deliverScene(scene);
                } catch (error) {
                    reportError(error);
                }
            },
            renderView(view) {
                pendingView = view;
                if (!backend) return;
                try {
                    post({ type: "view", view });
                } catch (error) {
                    reportError(error);
                }
            },
            command(name) {
                if (!backend) return;
                try {
                    post({ type: "command", name });
                } catch (error) {
                    reportError(error);
                }
            },
            destroy() {
                if (animationFrame) cancelAnimationFrame(animationFrame);
                animationFrame = 0;
                framePending = false;
                removeInputListeners?.();
                removeInputListeners = null;
                removeShortcutListener?.();
                removeShortcutListener = null;
                resizeObserver?.disconnect();
                resizeObserver = null;
                if (backend?.kind === "worker") {
                    backend.worker.postMessage({ type: "destroy" });
                    backend.worker.terminate();
                } else if (backend?.kind === "main") {
                    backend.handle.destroy();
                }
                backend = null;
                if (window.bpEditorCanvas === runtime) window.bpEditorCanvas = null;
            },
        };
        window.bpEditorCanvas = runtime;
        return runtime;
    }

    function installInput(canvas) {
        const removers = [];
        const listen = (target, name, callback, options) => {
            target.addEventListener(name, callback, options);
            removers.push(() => target.removeEventListener(name, callback, options));
        };
        const localPoint = (event) => {
            const rect = canvas.getBoundingClientRect();
            return [event.clientX - rect.left, event.clientY - rect.top];
        };
        const modifiers = (event) => ({
            alt: event.altKey,
            ctrl: event.ctrlKey,
            shift: event.shiftKey,
            command: event.metaKey,
        });
        let queuedPointerMove = null;
        let pointerFrame = 0;

        const flushPointerMove = () => {
            pointerFrame = 0;
            if (!queuedPointerMove) return;
            post(queuedPointerMove);
            queuedPointerMove = null;
        };
        const forwardPointer = (kind, event, coalesce = false) => {
            if (!backend) return;
            const [x, y] = localPoint(event);
            const message = {
                type: "pointer",
                kind,
                x,
                y,
                button: event.button,
                detail: event.detail || 0,
                source: event.pointerType || "mouse",
                ...modifiers(event),
            };
            if (coalesce) {
                queuedPointerMove = message;
                if (!pointerFrame) pointerFrame = requestAnimationFrame(flushPointerMove);
            } else {
                if (kind !== "enter" && kind !== "leave") flushPointerMove();
                post(message);
            }
        };

        const removeTouchNavigation = installCanvasTouchNavigation({
            canvas,
            listen,
            localPoint,
            modifiers,
            send: post,
            forwardPointer,
        });
        listen(canvas, "contextmenu", (event) => event.preventDefault());
        listen(canvas, "wheel", (event) => {
            event.preventDefault();
            post({ type: "wheel", x: -event.deltaX, y: -event.deltaY });
        }, { passive: false });
        listen(canvas, "keydown", (event) => {
            post({ type: "key", key: event.key, ...modifiers(event) });
        });
        return () => {
            if (pointerFrame) cancelAnimationFrame(pointerFrame);
            removeTouchNavigation();
            removers.splice(0).forEach((remove) => remove());
        };
    }

    function requestMainFrame() {
        if (backend?.kind !== "main" || framePending) return;
        const { canvas, handle } = backend;
        framePending = true;
        animationFrame = requestAnimationFrame((timestamp) => {
            animationFrame = 0;
            framePending = false;
            if (backend?.kind !== "main" || backend.handle !== handle) return;
            try {
                canvas.style.cursor = handle.frame(timestamp);
                if (REPORT_FRAME_STATS) {
                    canvas.dataset.renderStats = handle.frame_stats();
                }
                if (handle.needs_repaint()) requestMainFrame();
            } catch (error) {
                reportError(error);
            }
        });
    }

    function waitForWorkerMessage(worker, type, timeoutMs = 10000) {
        return new Promise((resolve, reject) => {
            const timeout = setTimeout(() => {
                cleanup();
                reject(new Error(`Render Worker ${type} timed out`));
            }, timeoutMs);
            const onMessage = (event) => {
                if (event.data?.type === "error") {
                    cleanup();
                    reject(new Error(event.data.message || "Render Worker failed"));
                    return;
                }
                if (event.data?.type !== type) return;
                cleanup();
                resolve(event.data);
            };
            const onError = (event) => {
                cleanup();
                reject(new Error(event.message || "Render Worker failed"));
            };
            const cleanup = () => {
                clearTimeout(timeout);
                worker.removeEventListener("message", onMessage);
                worker.removeEventListener("error", onError);
            };
            worker.addEventListener("message", onMessage);
            worker.addEventListener("error", onError);
        });
    }

    function installWorkerEvents(worker, canvas) {
        worker.addEventListener("message", (event) => {
            const message = event.data ?? {};
            if (message.type === "renderer_event") send(message.event);
            else if (message.type === "cursor") canvas.style.cursor = message.cursor;
            else if (message.type === "frame_stats") {
                canvas.dataset.renderStats = message.stats;
            }
            else if (message.type === "warmed") {
                canvas.dataset.rendererWarm = message.error ? "error" : "ready";
                if (message.error) {
                    canvas.dataset.rendererWarmError = message.error;
                    console.warn("Renderer asset warm-up failed.", message.error);
                } else {
                    delete canvas.dataset.rendererWarmError;
                }
                document.documentElement.dataset.rendererWarm = canvas.dataset.rendererWarm;
            }
            else if (message.type === "error" && backend?.worker === worker) {
                reportError(message.message);
            }
        });
        worker.addEventListener("error", (event) => reportError(event.message));
    }

    async function startWorkerRenderer(canvas) {
        if (
            typeof Worker === "undefined" ||
            typeof canvas.transferControlToOffscreen !== "function"
        ) {
            return false;
        }
        const worker = new Worker(
            `${absoluteAsset("renderer/badpiggies-render-worker.js")}?v=${RENDERER_VERSION}`,
            { type: "module", name: "badpiggies-renderer" },
        );
        let transferred = false;
        try {
            const probePromise = waitForWorkerMessage(worker, "probe", 5000);
            worker.postMessage({ type: "probe" });
            const probe = await probePromise;
            if (!probe.supported) {
                worker.terminate();
                return false;
            }
            const size = canvasSize(canvas);
            const offscreen = canvas.transferControlToOffscreen();
            transferred = true;
            installWorkerEvents(worker, canvas);
            const readyPromise = waitForWorkerMessage(worker, "ready", 30000);
            worker.postMessage({
                type: "init",
                canvas: offscreen,
                assetRoot: absoluteAsset("").replace(/\/$/, ""),
                reportStats: REPORT_FRAME_STATS,
                ...size,
            }, [offscreen]);
            const ready = await readyPromise;
            backend = { kind: "worker", worker };
            canvas.dataset.rendererBackend = "worker";
            canvas.dataset.rendererWarm = "warming";
            canvas.dataset.rendererRuntime = ready.backend || "single";
            canvas.dataset.rendererThreads = String(ready.threadCount || 1);
            canvas.dataset.rendererFont = ready.fontBackend || "unavailable";
            document.documentElement.dataset.renderWorkerBackend = ready.backend || "single";
            document.documentElement.dataset.renderWorkerThreads = String(ready.threadCount || 1);
            document.documentElement.dataset.renderFontBackend =
                ready.fontBackend || "unavailable";
            return true;
        } catch (error) {
            worker.terminate();
            if (transferred) {
                const replacement = canvas.cloneNode(false);
                canvas.replaceWith(replacement);
                currentCanvas = replacement;
                runtime.canvas = replacement;
            }
            console.warn("Render Worker unavailable; using the main-thread renderer.", error);
            return false;
        }
    }

    async function startMainRenderer(canvas) {
        const moduleUrl = `${absoluteAsset("renderer/pkg/badpiggies_editor_renderer.js")}?v=${RENDERER_VERSION}`;
        const wasmUrl = `${absoluteAsset("renderer/pkg/badpiggies_editor_renderer_bg.wasm")}?v=${RENDERER_VERSION}`;
        const renderer = await importRendererModule(moduleUrl);
        canvas.dataset.rendererStage = "loading-wasm";
        await renderer.default({ module_or_path: wasmUrl });
        const handle = new renderer.RendererHandle();
        await handle.start(canvas, absoluteAsset("").replace(/\/$/, ""));
        backend = { kind: "main", handle, canvas };
        canvas.dataset.rendererBackend = "main";
        canvas.dataset.rendererRuntime = "single";
        canvas.dataset.rendererThreads = "1";
        canvas.dataset.rendererFont = handle.font_backend();
        document.documentElement.dataset.renderWorkerBackend = "main-single";
        document.documentElement.dataset.renderWorkerThreads = "1";
        document.documentElement.dataset.renderFontBackend = handle.font_backend();
        requestMainFrame();
        canvas.dataset.rendererWarm = "warming";
        window.setTimeout(() => {
            try {
                handle.warm_up();
                canvas.dataset.rendererWarm = "ready";
                document.documentElement.dataset.rendererWarm = "ready";
            } catch (error) {
                canvas.dataset.rendererWarm = "error";
                document.documentElement.dataset.rendererWarm = "error";
                console.warn("Renderer asset warm-up failed.", error);
            }
        }, 0);
    }

    function installResize(canvas) {
        const resize = () => post({ type: "resize", ...canvasSize(canvas) });
        resizeObserver = new ResizeObserver(resize);
        resizeObserver.observe(canvas);
        resize();
    }

    function installShortcuts() {
        const onKeyDown = (event) => {
            const target = event.target;
            if (target && (["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName) || target.isContentEditable)) return;
            const commandKey = event.ctrlKey || event.metaKey;
            let command = null;
            if (commandKey && event.key.toLowerCase() === "z") command = event.shiftKey ? "redo" : "undo";
            else if (commandKey && event.key.toLowerCase() === "y") command = "redo";
            else if (commandKey && event.key.toLowerCase() === "c") command = "copy";
            else if (commandKey && event.key.toLowerCase() === "x") command = "cut";
            else if (commandKey && event.key.toLowerCase() === "v") command = "paste";
            else if (commandKey && event.key.toLowerCase() === "d") command = "duplicate";
            else if (commandKey && event.key.toLowerCase() === "a") command = "select_all";
            else if (event.key === "Delete" || event.key === "Backspace") command = "delete";
            else if (event.key === "Escape") command = "deselect";
            else if (!commandKey && event.key.toLowerCase() === "v") command = "tool_select";
            else if (!commandKey && event.key.toLowerCase() === "m") command = "tool_box_select";
            else if (!commandKey && event.key.toLowerCase() === "p") command = "tool_draw_terrain";
            else if (!commandKey && event.key.toLowerCase() === "h") command = "tool_pan";
            else if (event.key.toLowerCase() === "f") {
                runtime?.command("fit");
                event.preventDefault();
                return;
            }
            if (command) {
                event.preventDefault();
                send({ type: "command", name: command });
            }
        };
        window.addEventListener("keydown", onKeyDown);
        return () => window.removeEventListener("keydown", onKeyDown);
    }

    async function attach() {
        const canvas = document.getElementById("editor-canvas");
        if (!canvas) {
            requestAnimationFrame(attach);
            return;
        }
        if (starting || backend) return;
        starting = true;
        currentCanvas = canvas;
        installRuntime(canvas);
        canvas.classList.add("renderer-loading");
        canvas.dataset.rendererStage = "loading-worker";

        try {
            const rendererMode = new URLSearchParams(window.location.search).get("renderer");
            const workerStarted = rendererMode === "main"
                ? false
                : await startWorkerRenderer(canvas);
            if (!workerStarted) {
                currentCanvas = runtime.canvas;
                currentCanvas.dataset.rendererStage = "loading-main-thread";
                await startMainRenderer(currentCanvas);
            }
            removeInputListeners = installInput(currentCanvas);
            removeShortcutListener = installShortcuts();
            installResize(currentCanvas);
            currentCanvas.classList.remove("renderer-loading", "renderer-error");
            currentCanvas.classList.add("renderer-ready");
            currentCanvas.dataset.rendererStage = "ready";
            if (pendingScene) deliverScene(pendingScene, true);
            if (pendingView) post({ type: "view", view: pendingView });
        } catch (error) {
            currentCanvas.classList.remove("renderer-loading");
            currentCanvas.classList.add("renderer-error");
            currentCanvas.dataset.rendererStage = "error";
            reportError(error);
        } finally {
            starting = false;
        }
    }

    const onBeforeUnload = () => runtime?.destroy();
    window.addEventListener("beforeunload", onBeforeUnload, { once: true });

    await attach();
    await new Promise(() => {});
})();
