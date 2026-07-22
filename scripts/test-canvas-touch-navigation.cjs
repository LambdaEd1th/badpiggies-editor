"use strict";

const assert = require("node:assert/strict");
const {
    installCanvasTouchNavigation,
} = require("../app/assets/canvas_touch_navigation.js");

function createHarness(touchEventMode = false) {
    const listeners = new Map();
    const messages = [];
    const forwarded = [];
    const frames = new Map();
    let nextFrame = 1;

    global.requestAnimationFrame = (callback) => {
        const id = nextFrame++;
        frames.set(id, callback);
        return id;
    };
    global.cancelAnimationFrame = (id) => frames.delete(id);

    const canvas = {
        dataset: {},
        focus() {},
        setPointerCapture() {},
    };
    const listen = (_target, name, callback) => listeners.set(name, callback);
    const cleanup = installCanvasTouchNavigation({
        canvas,
        listen,
        localPoint: (event) => [event.clientX, event.clientY],
        modifiers: () => ({ alt: false, ctrl: false, shift: false, command: false }),
        send: (message) => messages.push(message),
        touchEventMode,
        forwardPointer: (kind, event, coalesce = false) => {
            forwarded.push({ kind, pointerType: event.pointerType, coalesce });
        },
    });

    return {
        messages,
        forwarded,
        touchInput: canvas.dataset.touchInput,
        emit(name, options) {
            let prevented = false;
            const event = {
                pointerType: "touch",
                pointerId: 1,
                clientX: 0,
                clientY: 0,
                button: 0,
                detail: 0,
                preventDefault() {
                    prevented = true;
                },
                ...options,
            };
            listeners.get(name)(event);
            return prevented;
        },
        emitTouch(name, active = [], changed = []) {
            let prevented = false;
            const asTouchList = (items) => {
                const list = items.map(({ identifier, x, y }) => ({
                    identifier,
                    clientX: x,
                    clientY: y,
                }));
                list.item = (index) => list[index] ?? null;
                return list;
            };
            const event = {
                targetTouches: asTouchList(active),
                touches: asTouchList(active),
                changedTouches: asTouchList(changed),
                preventDefault() {
                    prevented = true;
                },
            };
            listeners.get(name)(event);
            return prevented;
        },
        frame() {
            const pending = Array.from(frames.values());
            frames.clear();
            pending.forEach((callback) => callback(0));
        },
        cleanup,
    };
}

const touch = (identifier, x, y) => ({ identifier, x, y });

{
    const harness = createHarness();
    assert.equal(harness.touchInput, "pointer-events");
    assert.equal(harness.emit("pointerdown", { clientX: 20, clientY: 30 }), true);
    assert.equal(harness.emit("pointerup", { clientX: 22, clientY: 32 }), true);
    assert.deepEqual(harness.messages.map(({ type, kind, x, y }) => ({ type, kind, x, y })), [
        { type: "pointer", kind: "down", x: 22, y: 32 },
        { type: "pointer", kind: "up", x: 22, y: 32 },
    ]);
    harness.cleanup();
}

{
    const harness = createHarness(true);
    assert.equal(harness.touchInput, "touch-events");
    assert.equal(harness.emitTouch("touchstart", [touch(1, 20, 30)], [touch(1, 20, 30)]), true);
    harness.emit("pointerdown", { pointerId: 1, clientX: 20, clientY: 30 });
    assert.deepEqual(harness.messages, [], "touch PointerEvents must not be processed twice");
    assert.equal(harness.emitTouch("touchend", [], [touch(1, 22, 32)]), true);
    assert.deepEqual(harness.messages.map(({ type, kind, x, y }) => ({ type, kind, x, y })), [
        { type: "pointer", kind: "down", x: 22, y: 32 },
        { type: "pointer", kind: "up", x: 22, y: 32 },
    ]);
    harness.cleanup();
}

{
    const harness = createHarness(true);
    harness.emitTouch("touchstart", [touch(1, 10, 15)], [touch(1, 10, 15)]);
    harness.emitTouch("touchmove", [touch(1, 24, 19)]);
    harness.frame();
    assert.deepEqual(harness.messages, [{
        type: "touch_transform",
        zoom: 1,
        dx: 14,
        dy: 4,
        x: 24,
        y: 19,
    }]);
    harness.emitTouch("touchend", [], [touch(1, 24, 19)]);
    assert.equal(harness.messages.length, 1);
    harness.cleanup();
}

{
    const harness = createHarness(true);
    harness.emitTouch("touchstart", [touch(1, 100, 100)], [touch(1, 100, 100)]);
    harness.emitTouch(
        "touchstart",
        [touch(1, 100, 100), touch(2, 200, 100)],
        [touch(2, 200, 100)],
    );
    harness.emitTouch("touchmove", [touch(1, 90, 100), touch(2, 210, 100)]);
    harness.frame();
    assert.deepEqual(harness.messages, [{
        type: "touch_transform",
        zoom: 1.2,
        dx: 0,
        dy: 0,
        x: 150,
        y: 100,
    }]);

    harness.emitTouch("touchend", [touch(1, 90, 100)], [touch(2, 210, 100)]);
    harness.emitTouch("touchmove", [touch(1, 80, 95)]);
    harness.frame();
    assert.deepEqual(harness.messages[1], {
        type: "touch_transform",
        zoom: 1,
        dx: -10,
        dy: -5,
        x: 80,
        y: 95,
    });
    harness.cleanup();
}

{
    const harness = createHarness(true);
    harness.emitTouch(
        "touchstart",
        [touch(1, 100, 100), touch(2, 200, 100)],
        [touch(1, 100, 100), touch(2, 200, 100)],
    );
    harness.emitTouch("touchmove", [touch(2, 220, 100), touch(1, 80, 100)]);
    harness.frame();
    assert.equal(harness.messages[0].zoom, 1.4);
    assert.equal(harness.messages[0].x, 150);
    harness.emitTouch("touchcancel", [], [touch(1, 80, 100), touch(2, 220, 100)]);
    assert.equal(harness.messages.filter(({ type }) => type === "pointer").length, 0);
    harness.cleanup();
}

{
    const harness = createHarness();
    harness.emit("pointerdown", { clientX: 10, clientY: 15 });
    harness.emit("pointermove", { clientX: 24, clientY: 19 });
    harness.frame();
    assert.deepEqual(harness.messages, [{
        type: "touch_transform",
        zoom: 1,
        dx: 14,
        dy: 4,
        x: 24,
        y: 19,
    }]);
    harness.emit("pointerup", { clientX: 24, clientY: 19 });
    assert.equal(harness.messages.length, 1);
    harness.cleanup();
}

{
    const harness = createHarness();
    harness.emit("pointerdown", { pointerId: 1, clientX: 100, clientY: 100 });
    harness.emit("pointerdown", { pointerId: 2, clientX: 200, clientY: 100 });
    harness.emit("pointermove", { pointerId: 2, clientX: 220, clientY: 100 });
    harness.frame();
    assert.deepEqual(harness.messages, [{
        type: "touch_transform",
        zoom: 1.2,
        dx: 10,
        dy: 0,
        x: 160,
        y: 100,
    }]);

    harness.emit("pointerup", { pointerId: 2, clientX: 220, clientY: 100 });
    harness.emit("pointermove", { pointerId: 1, clientX: 90, clientY: 95 });
    harness.frame();
    assert.deepEqual(harness.messages[1], {
        type: "touch_transform",
        zoom: 1,
        dx: -10,
        dy: -5,
        x: 90,
        y: 95,
    });
    harness.cleanup();
}

{
    const harness = createHarness();
    harness.emit("pointerdown", { pointerType: "mouse" });
    harness.emit("pointermove", { pointerType: "mouse" });
    harness.emit("pointerup", { pointerType: "mouse" });
    assert.deepEqual(harness.forwarded, [
        { kind: "down", pointerType: "mouse", coalesce: false },
        { kind: "move", pointerType: "mouse", coalesce: true },
        { kind: "up", pointerType: "mouse", coalesce: false },
    ]);
    assert.deepEqual(harness.messages, []);
    harness.cleanup();
}

console.log("canvas touch navigation tests passed");
