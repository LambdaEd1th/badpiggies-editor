return (async function () {
    window.bpEditorCanvas?.destroy();
    window.bpContraptionPreview?.destroy();

    const canvas = document.getElementById("editor-canvas");
    if (!canvas) {
        dioxus.send({ type: "error", message: "Native canvas element is unavailable" });
        return;
    }

    const send = (message) => {
        try {
            dioxus.send(message);
        } catch (_) {
            // The evaluator may be shutting down with its component.
        }
    };
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

    let resizeObserver = null;
    let pointerFrame = 0;
    let queuedPointerMove = null;

    const flushPointerMove = () => {
        pointerFrame = 0;
        if (!queuedPointerMove) return;
        send(queuedPointerMove);
        queuedPointerMove = null;
    };
    const forwardPointer = (kind, event, coalesce = false) => {
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
            send(message);
        }
    };

    const removeTouchNavigation = installCanvasTouchNavigation({
        canvas,
        listen,
        localPoint,
        modifiers,
        send,
        forwardPointer,
    });
    listen(canvas, "contextmenu", (event) => event.preventDefault());
    listen(canvas, "wheel", (event) => {
        event.preventDefault();
        send({ type: "wheel", x: -event.deltaX, y: -event.deltaY });
    }, { passive: false });
    listen(canvas, "keydown", (event) => {
        send({ type: "key", key: event.key, ...modifiers(event) });
    });

    const onShortcut = (event) => {
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
        else if (event.key.toLowerCase() === "f") command = "fit";
        if (command) {
            event.preventDefault();
            send({ type: "command", name: command });
        }
    };
    listen(window, "keydown", onShortcut);

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
    resizeObserver = new ResizeObserver(reportBounds);
    resizeObserver.observe(canvas);
    listen(window, "resize", reportBounds);

    const runtime = {
        setCursor(cursor) {
            canvas.style.cursor = cursor || "default";
        },
        destroy() {
            if (pointerFrame) cancelAnimationFrame(pointerFrame);
            pointerFrame = 0;
            removeTouchNavigation();
            resizeObserver?.disconnect();
            resizeObserver = null;
            removers.splice(0).forEach((remove) => remove());
            if (window.bpEditorCanvas === runtime) window.bpEditorCanvas = null;
        },
    };
    window.bpEditorCanvas = runtime;
    reportBounds();
    send({ type: "ready" });
    await new Promise(() => {});
})();
