function installCanvasTouchNavigation({
    canvas,
    listen,
    localPoint,
    modifiers,
    send,
    forwardPointer,
    touchEventMode,
    scheduleTimeout = setTimeout,
    cancelTimeout = clearTimeout,
    longPressDelay = 500,
}) {
    const DIRECT_DRAG_THRESHOLD_SQ = 64;
    const useTouchEvents = touchEventMode ?? (
        typeof window !== "undefined" && (
            "ontouchstart" in window ||
            (typeof navigator !== "undefined" && navigator.maxTouchPoints > 0)
        )
    );
    const touches = new Map();
    let touchSequence = null;
    let longPressTimer = null;
    let gestureBaseline = null;
    let latestGesture = null;
    let gestureFrame = 0;
    let directPointerMove = null;
    let directPointerFrame = 0;
    let lastDirectPointerPoint = null;

    if (canvas.dataset) {
        canvas.dataset.touchInput = useTouchEvents ? "touch-events" : "pointer-events";
    }

    const eventModifiers = (event) => {
        const values = modifiers(event);
        return {
            alt: Boolean(values.alt),
            ctrl: Boolean(values.ctrl),
            shift: Boolean(values.shift),
            command: Boolean(values.command),
        };
    };

    const sendPointerAt = (kind, point, values, button = 0, detail = 0) => {
        send({
            type: "pointer",
            kind,
            x: point[0],
            y: point[1],
            button,
            detail,
            source: "touch",
            ...values,
        });
    };

    const clearLongPress = () => {
        if (longPressTimer !== null) cancelTimeout(longPressTimer);
        longPressTimer = null;
    };

    const distanceFromStartSq = (point) => {
        if (!touchSequence || !point) return 0;
        const dx = point[0] - touchSequence.start[0];
        const dy = point[1] - touchSequence.start[1];
        return dx * dx + dy * dy;
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

    const flushDirectPointerMove = () => {
        if (directPointerFrame) cancelAnimationFrame(directPointerFrame);
        directPointerFrame = 0;
        if (!directPointerMove) return;
        const { point, values } = directPointerMove;
        directPointerMove = null;
        sendPointerAt("move", point, values);
        lastDirectPointerPoint = point;
    };

    const queueDirectPointerMove = (point, values) => {
        const queuedPoint = directPointerMove?.point ?? lastDirectPointerPoint;
        if (queuedPoint && queuedPoint[0] === point[0] && queuedPoint[1] === point[1]) return;
        directPointerMove = { point, values };
        if (!directPointerFrame) {
            directPointerFrame = requestAnimationFrame(() => {
                directPointerFrame = 0;
                flushDirectPointerMove();
            });
        }
    };

    const scheduleLongPressMenu = () => {
        clearLongPress();
        const sequence = touchSequence;
        longPressTimer = scheduleTimeout(() => {
            longPressTimer = null;
            if (touchSequence !== sequence || sequence?.mode !== "pending" || touches.size !== 1) {
                return;
            }
            const point = touches.get(sequence.identifier);
            if (!point || distanceFromStartSq(point) > DIRECT_DRAG_THRESHOLD_SQ) return;
            sequence.mode = "consumed";
            sendPointerAt("down", point, sequence.modifiers, 2, 1);
            sendPointerAt("up", point, sequence.modifiers, 2, 1);
        }, longPressDelay);
    };

    const beginSequence = (identifier, point, event) => {
        if (touchSequence || !point) return;
        touchSequence = {
            identifier,
            start: point,
            mode: "pending",
            modifiers: eventModifiers(event),
        };
        scheduleLongPressMenu();
    };

    const beginDirectPointer = (point, event) => {
        if (!touchSequence || touchSequence.mode !== "pending") return;
        clearLongPress();
        touchSequence.mode = "direct";
        touchSequence.modifiers = eventModifiers(event);
        lastDirectPointerPoint = touchSequence.start;
        sendPointerAt("down", touchSequence.start, touchSequence.modifiers, 0, 1);
        queueDirectPointerMove(point, touchSequence.modifiers);
    };

    const enterViewportGesture = (event) => {
        if (!touchSequence || touchSequence.mode === "viewport" || touchSequence.mode === "consumed") {
            return;
        }
        clearLongPress();
        if (touchSequence.mode === "direct") {
            const point = touches.get(touchSequence.identifier) ?? touchSequence.start;
            flushDirectPointerMove();
            sendPointerAt("cancel", point, eventModifiers(event));
        }
        touchSequence.mode = "viewport";
        resetTouchBaseline();
    };

    const handlePrimaryMovement = (event) => {
        if (!touchSequence) return;
        if (touchSequence.mode === "viewport") {
            queueTouchTransform();
            return;
        }
        if (touchSequence.mode === "consumed") return;
        const point = touches.get(touchSequence.identifier);
        if (!point) return;
        if (touchSequence.mode === "pending") {
            if (distanceFromStartSq(point) <= DIRECT_DRAG_THRESHOLD_SQ) return;
            beginDirectPointer(point, event);
            return;
        }
        touchSequence.modifiers = eventModifiers(event);
        queueDirectPointerMove(point, touchSequence.modifiers);
    };

    const finishPrimary = (event, point, allowTap) => {
        if (!touchSequence) return;
        clearLongPress();
        const values = eventModifiers(event);
        if (touchSequence.mode === "pending") {
            if (allowTap && distanceFromStartSq(point) <= DIRECT_DRAG_THRESHOLD_SQ) {
                sendPointerAt("down", point, values, 0, 1);
                sendPointerAt("up", point, values, 0, 1);
            } else if (allowTap) {
                beginDirectPointer(point, event);
                flushDirectPointerMove();
                sendPointerAt("up", point, values);
            }
        } else if (touchSequence.mode === "direct") {
            queueDirectPointerMove(point, values);
            flushDirectPointerMove();
            sendPointerAt(allowTap ? "up" : "cancel", point, values);
        }
        touchSequence.mode = "consumed";
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

    const finishRemovedTouches = () => {
        if (!touches.size) {
            clearLongPress();
            touchSequence = null;
            lastDirectPointerPoint = null;
            resetTouchBaseline();
            return;
        }
        if (touchSequence?.mode === "viewport") {
            resetTouchBaseline();
        } else if (touchSequence && !touches.has(touchSequence.identifier)) {
            touchSequence.mode = "viewport";
            resetTouchBaseline();
        }
    };

    const beginTouchEvent = (event) => {
        event.preventDefault();
        canvas.focus({ preventScroll: true });
        const active = activeTouchArray(event);
        const changed = touchArray(event.changedTouches);
        syncActiveTouches(active);
        if (!touchSequence) {
            const first = changed[0] ?? active[0];
            if (first) beginSequence(first.identifier, touches.get(first.identifier), event);
        }
        if (touches.size >= 2) enterViewportGesture(event);
    };

    const moveTouchEvent = (event) => {
        event.preventDefault();
        syncActiveTouches(activeTouchArray(event));
        handlePrimaryMovement(event);
    };

    const finishTouchEvent = (event, allowTap) => {
        event.preventDefault();
        const active = activeTouchArray(event);
        const changed = touchArray(event.changedTouches);
        updateTouchList(active);
        updateTouchList(changed);
        if (touchSequence?.mode === "viewport") {
            latestGesture = touchGeometry();
            flushTouchTransform();
        }
        const primaryChanged = changed.find(
            (touch) => touch.identifier === touchSequence?.identifier,
        );
        if (primaryChanged && touchSequence?.mode !== "viewport") {
            finishPrimary(event, localPoint(primaryChanged), allowTap);
        }
        syncActiveTouches(active);
        finishRemovedTouches();
    };

    const beginPointerTouch = (event) => {
        event.preventDefault();
        canvas.focus({ preventScroll: true });
        try {
            canvas.setPointerCapture(event.pointerId);
        } catch (_) {
            // Capture can fail if the browser already cancelled this pointer.
        }
        const point = localPoint(event);
        touches.set(event.pointerId, point);
        if (!touchSequence) beginSequence(event.pointerId, point, event);
        if (touches.size >= 2) enterViewportGesture(event);
    };

    const movePointerTouch = (event) => {
        if (!touches.has(event.pointerId)) return;
        event.preventDefault();
        touches.set(event.pointerId, localPoint(event));
        handlePrimaryMovement(event);
    };

    const finishPointerTouch = (event, allowTap) => {
        if (!touches.has(event.pointerId)) return;
        event.preventDefault();
        const point = localPoint(event);
        touches.set(event.pointerId, point);
        if (touchSequence?.mode === "viewport") {
            latestGesture = touchGeometry();
            flushTouchTransform();
        } else if (touchSequence?.identifier === event.pointerId) {
            finishPrimary(event, point, allowTap);
        }
        touches.delete(event.pointerId);
        finishRemovedTouches();
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
        clearLongPress();
        if (gestureFrame) cancelAnimationFrame(gestureFrame);
        if (directPointerFrame) cancelAnimationFrame(directPointerFrame);
        gestureFrame = 0;
        directPointerFrame = 0;
        directPointerMove = null;
        lastDirectPointerPoint = null;
        touches.clear();
        touchSequence = null;
        gestureBaseline = null;
        latestGesture = null;
    };
}

if (typeof module !== "undefined" && module.exports) {
    module.exports = { installCanvasTouchNavigation };
}
