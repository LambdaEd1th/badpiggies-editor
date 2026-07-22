function installCanvasTouchNavigation({
    canvas,
    listen,
    localPoint,
    modifiers,
    send,
    forwardPointer,
    touchEventMode,
}) {
    const TAP_MOVE_THRESHOLD_SQ = 36;
    const useTouchEvents = touchEventMode ?? (
        typeof window !== "undefined" && (
            "ontouchstart" in window ||
            (typeof navigator !== "undefined" && navigator.maxTouchPoints > 0)
        )
    );
    const touches = new Map();
    let touchSequence = null;
    let gestureBaseline = null;
    let latestGesture = null;
    let gestureFrame = 0;

    if (canvas.dataset) {
        canvas.dataset.touchInput = useTouchEvents ? "touch-events" : "pointer-events";
    }

    const sendPointerAt = (kind, point, event, detail = 0) => {
        send({
            type: "pointer",
            kind,
            x: point[0],
            y: point[1],
            button: 0,
            detail,
            ...modifiers(event),
        });
    };

    const touchGeometry = () => {
        const points = Array.from(touches.values()).slice(0, 2);
        if (!points.length) return null;
        if (points.length === 1) {
            return { count: 1, center: points[0], distance: 0 };
        }
        return {
            count: 2,
            center: [
                (points[0][0] + points[1][0]) / 2,
                (points[0][1] + points[1][1]) / 2,
            ],
            distance: Math.hypot(
                points[1][0] - points[0][0],
                points[1][1] - points[0][1],
            ),
        };
    };

    const flushTouchTransform = () => {
        if (gestureFrame) cancelAnimationFrame(gestureFrame);
        gestureFrame = 0;
        const previous = gestureBaseline;
        const current = latestGesture;
        if (!previous || !current || previous.count !== current.count) {
            gestureBaseline = current;
            return;
        }
        const zoom = current.count === 2 && previous.distance > 0
            ? current.distance / previous.distance
            : 1;
        const dx = current.center[0] - previous.center[0];
        const dy = current.center[1] - previous.center[1];
        if (Math.abs(zoom - 1) > 0.0001 || Math.abs(dx) > 0.01 || Math.abs(dy) > 0.01) {
            send({
                type: "touch_transform",
                zoom,
                dx,
                dy,
                x: current.center[0],
                y: current.center[1],
            });
        }
        gestureBaseline = current;
    };

    const queueTouchTransform = () => {
        latestGesture = touchGeometry();
        if (!gestureFrame) {
            gestureFrame = requestAnimationFrame(() => {
                gestureFrame = 0;
                flushTouchTransform();
            });
        }
    };

    const resetTouchBaseline = () => {
        if (gestureFrame) cancelAnimationFrame(gestureFrame);
        gestureFrame = 0;
        latestGesture = touchGeometry();
        gestureBaseline = latestGesture;
    };

    const updateNavigationThreshold = () => {
        if (!touchSequence || touchSequence.navigating) return;
        const primary = touches.get(touchSequence.identifier);
        if (!primary) return;
        const dx = primary[0] - touchSequence.start[0];
        const dy = primary[1] - touchSequence.start[1];
        touchSequence.navigating = dx * dx + dy * dy > TAP_MOVE_THRESHOLD_SQ;
    };

    const beginSequence = (identifier, point) => {
        if (!touchSequence) {
            touchSequence = {
                identifier,
                start: point,
                navigating: false,
                multi: false,
            };
        }
        if (touches.size >= 2) {
            touchSequence.multi = true;
            touchSequence.navigating = true;
        }
        resetTouchBaseline();
    };

    const finishSequence = (point, allowTap, activeCount) => {
        updateNavigationThreshold();
        if (touchSequence?.navigating) {
            latestGesture = touchGeometry();
            flushTouchTransform();
        }
        return Boolean(
            allowTap &&
            activeCount === 0 &&
            touches.size === 1 &&
            touchSequence &&
            !touchSequence.navigating &&
            !touchSequence.multi &&
            point,
        );
    };

    const touchArray = (list) => {
        const result = [];
        for (let index = 0; index < (list?.length ?? 0); index += 1) {
            result.push(list.item?.(index) ?? list[index]);
        }
        return result.filter(Boolean);
    };

    const activeTouchArray = (event) => touchArray(event.targetTouches ?? event.touches);

    const updateTouchList = (list) => {
        for (const touch of list) {
            touches.set(touch.identifier, localPoint(touch));
        }
    };

    const syncActiveTouches = (list) => {
        const active = new Set();
        for (const touch of list) {
            active.add(touch.identifier);
            touches.set(touch.identifier, localPoint(touch));
        }
        for (const identifier of touches.keys()) {
            if (!active.has(identifier)) touches.delete(identifier);
        }
    };

    const beginTouchEvent = (event) => {
        event.preventDefault();
        canvas.focus({ preventScroll: true });
        if (touchSequence?.navigating) flushTouchTransform();
        const active = activeTouchArray(event);
        syncActiveTouches(active);
        const first = active[0];
        if (!first) return;
        beginSequence(first.identifier, touches.get(first.identifier));
    };

    const moveTouchEvent = (event) => {
        event.preventDefault();
        syncActiveTouches(activeTouchArray(event));
        updateNavigationThreshold();
        if (touchSequence?.navigating) queueTouchTransform();
    };

    const finishTouchEvent = (event, allowTap) => {
        event.preventDefault();
        const active = activeTouchArray(event);
        const changed = touchArray(event.changedTouches);
        updateTouchList(active);
        updateTouchList(changed);
        const primaryChanged = changed.find(
            (touch) => touch.identifier === touchSequence?.identifier,
        );
        const point = primaryChanged
            ? localPoint(primaryChanged)
            : touches.get(touchSequence?.identifier);
        const isTap = finishSequence(point, allowTap, active.length);
        syncActiveTouches(active);
        if (touches.size) {
            if (touchSequence) {
                touchSequence.multi = true;
                touchSequence.navigating = true;
            }
        } else {
            touchSequence = null;
        }
        resetTouchBaseline();
        if (isTap) {
            sendPointerAt("down", point, event, 1);
            sendPointerAt("up", point, event, 1);
        }
    };

    const beginPointerTouch = (event) => {
        event.preventDefault();
        canvas.focus({ preventScroll: true });
        try {
            canvas.setPointerCapture(event.pointerId);
        } catch (_) {
            // Capture can fail if the browser already cancelled this pointer.
        }
        if (touchSequence?.navigating) flushTouchTransform();
        const point = localPoint(event);
        touches.set(event.pointerId, point);
        beginSequence(event.pointerId, point);
    };

    const movePointerTouch = (event) => {
        if (!touches.has(event.pointerId)) return;
        event.preventDefault();
        touches.set(event.pointerId, localPoint(event));
        updateNavigationThreshold();
        if (touchSequence?.navigating) queueTouchTransform();
    };

    const finishPointerTouch = (event, allowTap) => {
        if (!touches.has(event.pointerId)) return;
        event.preventDefault();
        const point = localPoint(event);
        touches.set(event.pointerId, point);
        const isTap = finishSequence(point, allowTap, touches.size - 1);
        touches.delete(event.pointerId);
        if (touches.size) {
            if (touchSequence) {
                touchSequence.multi = true;
                touchSequence.navigating = true;
            }
        } else {
            touchSequence = null;
        }
        resetTouchBaseline();
        if (isTap) {
            sendPointerAt("down", point, event, 1);
            sendPointerAt("up", point, event, 1);
        }
    };

    if (useTouchEvents) {
        listen(canvas, "touchstart", beginTouchEvent, { passive: false });
        listen(canvas, "touchmove", moveTouchEvent, { passive: false });
        listen(canvas, "touchend", (event) => finishTouchEvent(event, true), { passive: false });
        listen(canvas, "touchcancel", (event) => finishTouchEvent(event, false), { passive: false });
    }

    listen(canvas, "pointerenter", (event) => {
        if (event.pointerType !== "touch") forwardPointer("enter", event);
    });
    listen(canvas, "pointerleave", (event) => {
        if (event.pointerType !== "touch") forwardPointer("leave", event);
    });
    listen(canvas, "pointerdown", (event) => {
        if (event.pointerType === "touch") {
            if (!useTouchEvents) beginPointerTouch(event);
            return;
        }
        event.preventDefault();
        canvas.focus({ preventScroll: true });
        try {
            canvas.setPointerCapture(event.pointerId);
        } catch (_) {
            // Capture can fail when the pointer is no longer active.
        }
        forwardPointer("down", event);
    }, { passive: false });
    listen(canvas, "pointermove", (event) => {
        if (event.pointerType === "touch") {
            if (!useTouchEvents) movePointerTouch(event);
            return;
        }
        forwardPointer("move", event, true);
    }, { passive: false });
    listen(canvas, "pointerup", (event) => {
        if (event.pointerType === "touch") {
            if (!useTouchEvents) finishPointerTouch(event, true);
        } else {
            forwardPointer("up", event);
        }
    }, { passive: false });
    listen(canvas, "pointercancel", (event) => {
        if (event.pointerType === "touch") {
            if (!useTouchEvents) finishPointerTouch(event, false);
        } else {
            forwardPointer("cancel", event);
        }
    }, { passive: false });
    listen(canvas, "lostpointercapture", (event) => {
        if (!useTouchEvents && event.pointerType === "touch" && touches.has(event.pointerId)) {
            finishPointerTouch(event, false);
        }
    });

    return () => {
        if (gestureFrame) cancelAnimationFrame(gestureFrame);
        gestureFrame = 0;
        touches.clear();
        touchSequence = null;
        gestureBaseline = null;
        latestGesture = null;
    };
}

if (typeof module !== "undefined") {
    module.exports = { installCanvasTouchNavigation };
}
