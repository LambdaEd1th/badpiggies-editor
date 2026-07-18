const ready = (async () => {
    const runtime = await import("./pkg/badpiggies_editor_worker.js");
    await runtime.default();
    return runtime;
})();

function responseTransferList(response) {
    const transfer = [];
    const visit = (value) => {
        if (!value || typeof value !== "object") return;
        if (value instanceof Uint8Array) {
            transfer.push(value.buffer);
            return;
        }
        if (Array.isArray(value)) {
            for (const item of value) visit(item);
            return;
        }
        for (const nested of Object.values(value)) visit(nested);
    };
    visit(response);
    return transfer;
}

self.onmessage = async (event) => {
    const id = event.data?.id;
    try {
        const runtime = await ready;
        const response = runtime.perform(event.data.request);
        self.postMessage(
            { id, ok: true, response },
            responseTransferList(response),
        );
    } catch (error) {
        self.postMessage({
            id,
            ok: false,
            error: error instanceof Error ? error.message : String(error),
        });
    }
};
