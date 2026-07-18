return (async function () {
    const RENDERER_VERSION = "20260718-contraption-preview-3";
    const DIOXUS_ASSET_ROOT = __BP_ASSET_ROOT__;
    window.bpContraptionPreview?.destroy();
    window.bpEditorCanvas?.destroy();

    const send = (message) => {
        try {
            dioxus.send(message);
        } catch (_) {
            // The evaluator may be shutting down with its component.
        }
    };

    let handle = null;
    let currentCanvas = null;
    let pendingPreview = null;
    let animationFrame = 0;
    let framePending = false;
    let resizeObserver = null;
    let disconnectObserver = null;
    let destroyed = false;
    let resolvedAssetRoot = null;

    function absoluteAsset(relative) {
        const root = resolvedAssetRoot ??
            new URL(DIOXUS_ASSET_ROOT, document.baseURI).href.replace(/\/$/, "");
        return relative ? `${root}/${relative}` : root;
    }

    async function resolveAssetRoot() {
        const root = new URL(DIOXUS_ASSET_ROOT, document.baseURI).href.replace(/\/$/, "");
        if (root.endsWith("/assets/assets")) return root;
        const bundledRoot = `${root}/assets`;
        try {
            const probe = await fetch(
                `${bundledRoot}/renderer/pkg/badpiggies_editor_renderer.js?v=${RENDERER_VERSION}`,
                { method: "HEAD", cache: "no-store" },
            );
            if (probe.ok || probe.status === 0) return bundledRoot;
        } catch (_) {
            // Native asset protocols use the direct root.
        }
        return root;
    }

    function canvasSize(canvas) {
        const rect = canvas.getBoundingClientRect();
        return {
            width: Math.max(1, Math.round(rect.width)),
            height: Math.max(1, Math.round(rect.height)),
        };
    }

    async function importRendererModule(moduleUrl) {
        const cacheKey = `renderer:${moduleUrl}`;
        window.bpRendererModulePromises ??= new Map();
        if (window.bpRendererModulePromises.has(cacheKey)) {
            return window.bpRendererModulePromises.get(cacheKey);
        }
        const promise = (async () => {
            try {
                return await import(moduleUrl);
            } catch (directError) {
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
                    return await import(blobUrl);
                } catch (fallbackError) {
                    throw new Error(
                        `Unable to load renderer module. Direct import: ${String(directError)}. ` +
                        `Blob fallback: ${String(fallbackError)}`,
                    );
                } finally {
                    URL.revokeObjectURL(blobUrl);
                }
            }
        })();
        window.bpRendererModulePromises.set(cacheKey, promise);
        return promise;
    }

    function resolveDarkMode(theme) {
        return theme === "dark" || (
            theme === "system" &&
            window.matchMedia?.("(prefers-color-scheme: dark)").matches
        );
    }

    function requestFrame() {
        if (!handle || framePending || destroyed) return;
        framePending = true;
        animationFrame = requestAnimationFrame((timestamp) => {
            animationFrame = 0;
            framePending = false;
            if (!handle || destroyed) return;
            try {
                handle.frame(timestamp);
                if (handle.needs_repaint()) requestFrame();
            } catch (error) {
                reportError(error);
            }
        });
    }

    function deliverPreview(payload) {
        pendingPreview = payload;
        if (!handle) return;
        handle.set_contraption_preview({
            parts: payload.parts,
            dark_mode: resolveDarkMode(payload.theme),
        });
        requestFrame();
    }

    function reportError(error) {
        const message = error instanceof Error ? error.message : String(error);
        console.error("Bad Piggies contraption preview:", error);
        currentCanvas?.classList.remove("renderer-loading", "renderer-ready");
        currentCanvas?.classList.add("renderer-error");
        send({ type: "error", message: `Contraption preview failed: ${message}` });
    }

    const runtime = {
        render: deliverPreview,
        destroy() {
            destroyed = true;
            if (animationFrame) cancelAnimationFrame(animationFrame);
            animationFrame = 0;
            framePending = false;
            resizeObserver?.disconnect();
            resizeObserver = null;
            disconnectObserver?.disconnect();
            disconnectObserver = null;
            handle?.destroy();
            handle = null;
            if (window.bpContraptionPreview === runtime) {
                window.bpContraptionPreview = null;
            }
        },
    };
    window.bpContraptionPreview = runtime;

    try {
        currentCanvas = document.getElementById("contraption-preview-canvas");
        if (!currentCanvas) throw new Error("preview canvas is unavailable");
        currentCanvas.classList.add("renderer-loading");
        disconnectObserver = new MutationObserver(() => {
            if (!currentCanvas?.isConnected) runtime.destroy();
        });
        disconnectObserver.observe(document.body, { childList: true, subtree: true });
        resolvedAssetRoot = await resolveAssetRoot();

        const moduleUrl = `${absoluteAsset("renderer/pkg/badpiggies_editor_renderer.js")}?v=${RENDERER_VERSION}`;
        const wasmUrl = `${absoluteAsset("renderer/pkg/badpiggies_editor_renderer_bg.wasm")}?v=${RENDERER_VERSION}`;
        const renderer = await importRendererModule(moduleUrl);
        await renderer.default({ module_or_path: wasmUrl });
        if (destroyed) return;

        handle = new renderer.RendererHandle();
        await handle.start(currentCanvas, absoluteAsset("").replace(/\/$/, ""));
        if (destroyed) {
            handle.destroy();
            handle = null;
            return;
        }

        const resize = () => {
            if (!handle) return;
            const size = canvasSize(currentCanvas);
            handle.resize(size.width, size.height);
            requestFrame();
        };
        resizeObserver = new ResizeObserver(resize);
        resizeObserver.observe(currentCanvas);
        resize();
        if (pendingPreview) deliverPreview(pendingPreview);

        currentCanvas.classList.remove("renderer-loading", "renderer-error");
        currentCanvas.classList.add("renderer-ready");
        send({ type: "ready" });
    } catch (error) {
        reportError(error);
    }

    await new Promise(() => {});
})();
