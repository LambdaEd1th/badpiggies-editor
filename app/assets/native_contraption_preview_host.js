return (async function () {
    window.bpEditorCanvas?.destroy();
    window.bpContraptionPreview?.destroy();

    const canvas = document.getElementById("contraption-preview-canvas");
    if (!canvas) {
        dioxus.send({ type: "error", message: "Native contraption preview canvas is unavailable" });
        return;
    }
    const send = (message) => {
        try {
            dioxus.send(message);
        } catch (_) {
            // The evaluator may be shutting down with its component.
        }
    };
    const reportBounds = () => {
        const rect = canvas.getBoundingClientRect();
        send({
            type: "bounds",
            x: rect.left,
            y: rect.top,
            width: Math.max(1, rect.width),
            height: Math.max(1, rect.height),
            window_width: Math.max(1, window.innerWidth),
            window_height: Math.max(1, window.innerHeight),
        });
    };
    const media = window.matchMedia?.("(prefers-color-scheme: dark)");
    const reportAppearance = () => send({ type: "appearance", dark_mode: !!media?.matches });
    const resizeObserver = new ResizeObserver(reportBounds);
    resizeObserver.observe(canvas);
    window.addEventListener("resize", reportBounds);
    media?.addEventListener?.("change", reportAppearance);

    const runtime = {
        destroy() {
            resizeObserver.disconnect();
            window.removeEventListener("resize", reportBounds);
            media?.removeEventListener?.("change", reportAppearance);
            if (window.bpContraptionPreview === runtime) {
                window.bpContraptionPreview = null;
            }
        },
    };
    window.bpContraptionPreview = runtime;
    reportBounds();
    reportAppearance();
    send({ type: "ready" });
    await new Promise(() => {});
})();
