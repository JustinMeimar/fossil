const viewer = document.querySelector('.pdf-viewer');
const status = document.querySelector('#pdf-status');
const scroll = viewer.querySelector('.pdf-scroll');
const canvas = document.querySelector('#pdf-canvas');
const pageInput = document.querySelector('#pdf-page');
const previous = document.querySelector('#pdf-prev');
const next = document.querySelector('#pdf-next');
const zoomIn = document.querySelector('#pdf-in');
const zoomOut = document.querySelector('#pdf-out');
const fit = document.querySelector('#pdf-fit');
let documentPDF, renderTask, pageNumber = 1, scale = null, revision = 0;

async function render() {
    const request = ++revision;
    renderTask?.cancel();
    viewer.setAttribute('aria-busy', 'true');
    try {
        const page = await documentPDF.getPage(pageNumber);
        if (request !== revision) return;
        const width = page.getViewport({scale: 1}).width;
        const zoom = scale ?? Math.max(0.1, (scroll.clientWidth - 32) / width);
        const viewport = page.getViewport({scale: zoom});
        const ratio = Math.min(window.devicePixelRatio || 1, 2, 8192 / Math.max(viewport.width, viewport.height));
        await renderTask?.promise.catch(() => {});
        if (request !== revision) return;
        canvas.width = Math.ceil(viewport.width * ratio);
        canvas.height = Math.ceil(viewport.height * ratio);
        canvas.style.width = `${viewport.width}px`;
        canvas.style.height = `${viewport.height}px`;
        canvas.setAttribute('aria-label', `PDF page ${pageNumber} of ${documentPDF.numPages}; readable text follows below`);
        document.querySelector('#pdf-zoom').value = `${Math.round(zoom * 100)}%`;
        previous.disabled = pageNumber === 1;
        next.disabled = pageNumber === documentPDF.numPages;
        pageInput.value = pageNumber;
        renderTask = page.render({canvasContext: canvas.getContext('2d'), viewport,
            transform: [ratio, 0, 0, ratio, 0, 0]});
        await renderTask.promise;
        if (request !== revision) return;
        const text = await page.getTextContent();
        if (request !== revision) return;
        document.querySelector('#pdf-text').textContent = text.items.map(item => item.str + (item.hasEOL ? '\n' : ' ')).join('');
        status.textContent = '';
    } catch (error) {
        if (request !== revision || error.name === 'RenderingCancelledException') return;
        status.textContent = `Could not display this PDF. Use Open PDF or Download. ${error.message}`;
    } finally {
        if (request === revision) viewer.setAttribute('aria-busy', 'false');
    }
}
function move(page) {
    pageNumber = Math.max(1, Math.min(documentPDF.numPages, page));
    scroll.scrollTo(0, 0);
    render();
}
function zoom(factor) {
    const current = scale ?? Number.parseInt(document.querySelector('#pdf-zoom').value, 10) / 100;
    scale = Math.max(0.25, Math.min(4, current * factor));
    render();
}
previous.addEventListener('click', () => move(pageNumber - 1));
next.addEventListener('click', () => move(pageNumber + 1));
pageInput.addEventListener('change', () => move(Number.parseInt(pageInput.value, 10) || 1));
zoomIn.addEventListener('click', () => zoom(1.2));
zoomOut.addEventListener('click', () => zoom(1 / 1.2));
fit.addEventListener('click', () => { scale = null; render(); });
let resizeTimer;
new ResizeObserver(() => {
    clearTimeout(resizeTimer);
    if (documentPDF && scale === null) resizeTimer = setTimeout(render, 100);
}).observe(scroll);
try {
    const {getDocument, GlobalWorkerOptions} = await import('/vendor/pdf.mjs');
    GlobalWorkerOptions.workerSrc = '/vendor/pdf.worker.mjs';
    documentPDF = await getDocument({url: viewer.dataset.url}).promise;
    document.querySelector('#pdf-pages').textContent = `of ${documentPDF.numPages}`;
    pageInput.max = documentPDF.numPages;
    [pageInput, zoomIn, zoomOut, fit].forEach(control => control.disabled = false);
    await render();
} catch (error) {
    status.textContent = `Could not load this PDF. Use Open PDF or Download. ${error.message}`;
}
