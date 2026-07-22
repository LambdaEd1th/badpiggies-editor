(() => {
    window.bpWorkspacePanels?.dispose?.();

    const DESKTOP_QUERY = "(min-width: 901px)";
    const STORAGE_KEYS = {
        left: "badpiggies-editor-left-panel-width",
        right: "badpiggies-editor-right-panel-width",
    };
    const PROPERTIES = {
        left: "--rton-left-panel-width",
        right: "--rton-right-panel-width",
    };
    const MIN_WIDTH = { left: 200, right: 260 };
    const MAX_WIDTH = { left: 480, right: 560 };
    const FALLBACK_WIDTH = { left: 260, right: 330 };
    const MIN_CENTER_WIDTH = 300;
    const HANDLE_TRACK_WIDTH = 24;

    let activeDrag = null;

    function shellFor(element) {
        return element?.closest?.(".rton-workspace-shell") ?? null;
    }

    function sideFor(handle) {
        return handle.classList.contains("rton-resize-handle-left") ? "left" : "right";
    }

    function panelFor(shell, side) {
        return shell.querySelector(`.rton-side-panel-${side}`);
    }

    function panelWidth(shell, side) {
        const width = panelFor(shell, side)?.getBoundingClientRect().width;
        return Number.isFinite(width) && width > 0 ? width : FALLBACK_WIDTH[side];
    }

    function limits(shell, side) {
        const opposite = side === "left" ? "right" : "left";
        const available = shell.getBoundingClientRect().width
            - panelWidth(shell, opposite)
            - MIN_CENTER_WIDTH
            - HANDLE_TRACK_WIDTH;
        return {
            min: MIN_WIDTH[side],
            max: Math.max(MIN_WIDTH[side], Math.min(MAX_WIDTH[side], available)),
        };
    }

    function clampWidth(shell, side, width) {
        const { min, max } = limits(shell, side);
        return Math.round(Math.min(max, Math.max(min, width)));
    }

    function updateHandle(handle) {
        const shell = shellFor(handle);
        if (!shell) return;
        const side = sideFor(handle);
        const { min, max } = limits(shell, side);
        handle.setAttribute("aria-valuemin", String(Math.round(min)));
        handle.setAttribute("aria-valuemax", String(Math.round(max)));
        handle.setAttribute("aria-valuenow", String(Math.round(panelWidth(shell, side))));
    }

    function updateHandles(shell) {
        shell.querySelectorAll(".rton-resize-handle").forEach(updateHandle);
    }

    function setWidth(shell, side, width, persist = false) {
        const next = clampWidth(shell, side, width);
        shell.style.setProperty(PROPERTIES[side], `${next}px`);
        updateHandles(shell);
        if (persist) {
            try {
                localStorage.setItem(STORAGE_KEYS[side], String(next));
            } catch (_) {
                // Width persistence is optional in restricted browser contexts.
            }
        }
        return next;
    }

    function storedWidth(side) {
        try {
            const value = Number(localStorage.getItem(STORAGE_KEYS[side]));
            return Number.isFinite(value) && value > 0 ? value : null;
        } catch (_) {
            return null;
        }
    }

    function restore(shell) {
        if (!matchMedia(DESKTOP_QUERY).matches) return;
        const left = storedWidth("left");
        const right = storedWidth("right");
        if (right !== null) setWidth(shell, "right", right);
        if (left !== null) setWidth(shell, "left", left);
        if (right !== null) setWidth(shell, "right", right);
        updateHandles(shell);
    }

    function beginDrag(event, handle) {
        if (!matchMedia(DESKTOP_QUERY).matches || event.button !== 0) return;
        const shell = shellFor(handle);
        if (!shell) return;
        event.preventDefault();
        activeDrag = { pointerId: event.pointerId, shell, side: sideFor(handle), handle };
        handle.setPointerCapture?.(event.pointerId);
        shell.classList.add("panel-resizing");
        document.documentElement.classList.add("rton-panel-resizing");
    }

    function moveDrag(event) {
        if (!activeDrag || event.pointerId !== activeDrag.pointerId) return;
        const bounds = activeDrag.shell.getBoundingClientRect();
        const width = activeDrag.side === "left"
            ? event.clientX - bounds.left
            : bounds.right - event.clientX;
        setWidth(activeDrag.shell, activeDrag.side, width);
        event.preventDefault();
    }

    function finishDrag(event) {
        if (!activeDrag || (event && event.pointerId !== activeDrag.pointerId)) return;
        const { shell, side, handle, pointerId } = activeDrag;
        activeDrag = null;
        handle.releasePointerCapture?.(pointerId);
        shell.classList.remove("panel-resizing");
        document.documentElement.classList.remove("rton-panel-resizing");
        setWidth(shell, side, panelWidth(shell, side), true);
    }

    function resetWidth(handle) {
        const shell = shellFor(handle);
        if (!shell) return;
        const side = sideFor(handle);
        shell.style.removeProperty(PROPERTIES[side]);
        try {
            localStorage.removeItem(STORAGE_KEYS[side]);
        } catch (_) {
            // Width persistence is optional in restricted browser contexts.
        }
        updateHandles(shell);
    }

    function resizeFromKeyboard(event, handle) {
        if (!matchMedia(DESKTOP_QUERY).matches) return;
        const shell = shellFor(handle);
        if (!shell) return;
        const side = sideFor(handle);
        const { min, max } = limits(shell, side);
        const step = event.shiftKey ? 24 : 8;
        let width = panelWidth(shell, side);
        if (event.key === "Home") width = min;
        else if (event.key === "End") width = max;
        else if (event.key === "ArrowLeft") width += side === "right" ? step : -step;
        else if (event.key === "ArrowRight") width += side === "left" ? step : -step;
        else return;
        event.preventDefault();
        setWidth(shell, side, width, true);
    }

    function onPointerDown(event) {
        const handle = event.target.closest?.(".rton-resize-handle");
        if (handle) beginDrag(event, handle);
    }

    function onDoubleClick(event) {
        const handle = event.target.closest?.(".rton-resize-handle");
        if (handle) resetWidth(handle);
    }

    function onKeyDown(event) {
        const handle = event.target.closest?.(".rton-resize-handle");
        if (handle) resizeFromKeyboard(event, handle);
    }

    function onWindowResize() {
        document.querySelectorAll(".rton-workspace-shell").forEach((shell) => {
            if (!matchMedia(DESKTOP_QUERY).matches) return;
            setWidth(shell, "left", panelWidth(shell, "left"));
            setWidth(shell, "right", panelWidth(shell, "right"));
        });
    }

    document.addEventListener("pointerdown", onPointerDown);
    document.addEventListener("pointermove", moveDrag, { passive: false });
    document.addEventListener("pointerup", finishDrag);
    document.addEventListener("pointercancel", finishDrag);
    document.addEventListener("dblclick", onDoubleClick);
    document.addEventListener("keydown", onKeyDown);
    window.addEventListener("resize", onWindowResize);

    requestAnimationFrame(() => {
        document.querySelectorAll(".rton-workspace-shell").forEach(restore);
    });

    window.bpWorkspacePanels = {
        dispose() {
            finishDrag();
            document.removeEventListener("pointerdown", onPointerDown);
            document.removeEventListener("pointermove", moveDrag);
            document.removeEventListener("pointerup", finishDrag);
            document.removeEventListener("pointercancel", finishDrag);
            document.removeEventListener("dblclick", onDoubleClick);
            document.removeEventListener("keydown", onKeyDown);
            window.removeEventListener("resize", onWindowResize);
        },
    };
})();
