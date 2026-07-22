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
    const MIN_WIDTH = 220;
    const MAX_WIDTH = 560;
    const FALLBACK_WIDTH = { left: 300, right: 380 };

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

    function clampWidth(width) {
        return Math.round(Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, width)));
    }

    function dragWidth(drag, clientX) {
        const delta = drag.side === "left"
            ? clientX - drag.startX
            : drag.startX - clientX;
        return clampWidth(drag.startWidth + delta);
    }

    function updateHandle(handle) {
        const shell = shellFor(handle);
        if (!shell) return;
        const side = sideFor(handle);
        handle.setAttribute("aria-valuemin", String(MIN_WIDTH));
        handle.setAttribute("aria-valuemax", String(MAX_WIDTH));
        handle.setAttribute("aria-valuenow", String(Math.round(panelWidth(shell, side))));
    }

    function updateHandles(shell) {
        shell.querySelectorAll(".rton-resize-handle").forEach(updateHandle);
    }

    function setWidth(shell, side, width, persist = false, updateHandleState = true) {
        const next = clampWidth(width);
        shell.style.setProperty(PROPERTIES[side], `${next}px`);
        if (updateHandleState) updateHandles(shell);
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
        if (left !== null) setWidth(shell, "left", left);
        if (right !== null) setWidth(shell, "right", right);
        updateHandles(shell);
    }

    function beginDrag(event, handle) {
        if (!matchMedia(DESKTOP_QUERY).matches || event.button !== 0) return;
        const shell = shellFor(handle);
        if (!shell) return;
        event.preventDefault();
        const side = sideFor(handle);
        const startWidth = panelWidth(shell, side);
        activeDrag = {
            pointerId: event.pointerId,
            shell,
            side,
            handle,
            startX: event.clientX,
            startWidth,
            pendingWidth: startWidth,
            frame: 0,
        };
        handle.setPointerCapture?.(event.pointerId);
        handle.classList.add("dragging");
        shell.classList.add("resizing-panel");
        document.documentElement.classList.add("rton-panel-resizing");
    }

    function moveDrag(event) {
        if (!activeDrag || event.pointerId !== activeDrag.pointerId) return;
        const drag = activeDrag;
        drag.pendingWidth = dragWidth(drag, event.clientX);
        if (!drag.frame) {
            drag.frame = requestAnimationFrame(() => {
                if (activeDrag !== drag) return;
                drag.frame = 0;
                setWidth(drag.shell, drag.side, drag.pendingWidth, false, false);
            });
        }
        event.preventDefault();
    }

    function finishDrag(event) {
        if (!activeDrag || (event && event.pointerId !== activeDrag.pointerId)) return;
        const drag = activeDrag;
        const { shell, side, handle, pointerId } = drag;
        activeDrag = null;
        if (drag.frame) cancelAnimationFrame(drag.frame);
        if (event && Number.isFinite(event.clientX)) {
            drag.pendingWidth = dragWidth(drag, event.clientX);
        }
        try {
            handle.releasePointerCapture?.(pointerId);
        } catch (_) {
            // Capture may already be released after a cancelled pointer.
        }
        handle.classList.remove("dragging");
        shell.classList.remove("resizing-panel");
        document.documentElement.classList.remove("rton-panel-resizing");
        setWidth(shell, side, drag.pendingWidth, true);
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
        const step = event.shiftKey ? 24 : 8;
        let width = panelWidth(shell, side);
        if (event.key === "Home") width = MIN_WIDTH;
        else if (event.key === "End") width = MAX_WIDTH;
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
