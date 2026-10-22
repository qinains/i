export function publishArchive({filename, bytes}, container) {
    const url = URL.createObjectURL(new Blob([bytes], {type:'application/zip'}));
    const link = container.ownerDocument.createElement('a');
    link.href = url; link.download = filename; link.textContent = `Save ${filename}`;
    container.append(link);
    let disposed = false;
    return {url, link, dispose() {
        if (disposed) return;
        disposed = true; URL.revokeObjectURL(url); link.remove();
    }};
}
