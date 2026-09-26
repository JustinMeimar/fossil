const busy = new Set();
function setBusy(task, active) {
    active ? busy.add(task) : busy.delete(task);
    const indicator = document.querySelector('#busy');
    if (indicator) indicator.hidden = busy.size === 0;
}

const divider = document.querySelector('#divider');
if (divider) {
    const workspace = divider.parentElement;
    function resize(percent) {
        const width = Math.max(20, Math.min(80, percent));
        workspace.style.setProperty('--records-width', `${width}fr`);
        workspace.style.setProperty('--viewer-width', `${100 - width}fr`);
        divider.setAttribute('aria-valuenow', String(Math.round(width)));
    }
    divider.addEventListener('pointerdown', event => {
        if (event.button !== 0) return;
        event.preventDefault();
        divider.focus();
        divider.setPointerCapture(event.pointerId);
        workspace.classList.add('resizing');
    });
    divider.addEventListener('pointermove', event => {
        if (!divider.hasPointerCapture(event.pointerId)) return;
        const bounds = workspace.getBoundingClientRect();
        resize(100 * (event.clientX - bounds.left - divider.offsetWidth / 2)
            / (bounds.width - divider.offsetWidth));
    });
    divider.addEventListener('pointerup', event => {
        if (divider.hasPointerCapture(event.pointerId)) divider.releasePointerCapture(event.pointerId);
    });
    divider.addEventListener('lostpointercapture', () => workspace.classList.remove('resizing'));
    divider.addEventListener('keydown', event => {
        const width = Number(divider.getAttribute('aria-valuenow'));
        const next = { ArrowLeft: width - 2, ArrowRight: width + 2, Home: 20, End: 80 }[event.key];
        if (next === undefined) return;
        event.preventDefault();
        resize(next);
    });
}

const tabs = [...document.querySelectorAll('[role=tab]')];
function selectTab(id) {
    tabs.forEach(tab => {
        const active = tab.id === `${id}-tab`;
        tab.setAttribute('aria-selected', String(active));
        tab.tabIndex = active ? 0 : -1;
        document.getElementById(tab.getAttribute('aria-controls')).hidden = !active;
    });
}
tabs.forEach((tab, index) => {
    tab.addEventListener('click', () => selectTab(tab.id.replace('-tab', '')));
    tab.addEventListener('keydown', event => {
        const next = { ArrowRight: (index + 1) % tabs.length,
            ArrowLeft: (index + tabs.length - 1) % tabs.length,
            Home: 0, End: tabs.length - 1 }[event.key];
        if (next === undefined) return;
        event.preventDefault();
        tabs[next].click();
        tabs[next].focus();
    });
});

const frame = document.querySelector('iframe[name=output]');
const output = document.querySelector('#analysis-output');
let pendingJob;
function viewOutput(target) {
    if (!frame) return;
    selectTab(target);
    setBusy(target, true);
    if (target !== 'output') return;
    output.hidden = true;
    frame.hidden = false;
}

document.addEventListener('click', event => {
    const link = event.target.closest('a');
    if (!link || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) return;
    if (['output', 'artifacts'].includes(link.target)) {
        (window === window.parent ? window : window.parent).viewOutput(link.target);
    } else {
        setBusy('navigation', true);
    }
});
window.addEventListener('pageshow', () => setBusy('navigation', false));
document.querySelectorAll('iframe').forEach(frame => {
    frame.addEventListener('load', () => setBusy(frame.name, false));
});

const form = document.querySelector('#analysis');
if (form) {
    const rows = [...document.querySelectorAll('#records tr')];
    const search = document.querySelector('#search');
    const variant = document.querySelector('#variant');
    const run = document.querySelector('#run');
    const count = document.querySelector('#selection-count');
    const selectVisible = document.querySelector('#select-visible');
    let running = false;
    const selected = () => rows.filter(row => row.querySelector('input').checked);
    function update() {
        const chosen = selected();
        const hidden = chosen.filter(row => row.hidden).length;
        count.textContent = `${chosen.length} selected${hidden ? ` (${hidden} hidden)` : ''}`;
        const visible = rows.filter(row => !row.hidden);
        const checked = visible.filter(row => row.querySelector('input').checked).length;
        if (selectVisible) {
            selectVisible.checked = visible.length > 0 && checked === visible.length;
            selectVisible.indeterminate = checked > 0 && checked < visible.length;
            selectVisible.disabled = visible.length === 0;
        }
        run.disabled = running || !chosen.length || !form.elements.analysis.value;
    }
    function filter() {
        const query = search.value.trim().toLowerCase();
        rows.forEach(row => {
            row.hidden = (variant.value && row.dataset.variant !== variant.value)
                || !row.textContent.toLowerCase().includes(query);
        });
        update();
    }
    search.addEventListener('input', filter);
    variant.addEventListener('change', filter);
    form.addEventListener('change', update);
    document.querySelector('#records')?.addEventListener('change', update);
    rows.forEach(row => row.addEventListener('click', event => {
        if (event.target.closest('a, input')) return;
        const checkbox = row.querySelector('input');
        checkbox.checked = !checkbox.checked;
        update();
    }));
    selectVisible?.addEventListener('change', () => {
        rows.filter(row => !row.hidden).forEach(row => row.querySelector('input').checked = selectVisible.checked);
        update();
    });
    document.querySelector('#clear-selection').addEventListener('click', () => {
        rows.forEach(row => row.querySelector('input').checked = false);
        update();
    });
    document.querySelectorAll('[data-sort]').forEach(button => {
        button.addEventListener('click', () => {
            const heading = button.closest('th');
            const direction = heading.getAttribute('aria-sort') === 'ascending' ? -1 : 1;
            document.querySelectorAll('th[aria-sort]').forEach(th => th.setAttribute('aria-sort', 'none'));
            heading.setAttribute('aria-sort', direction === 1 ? 'ascending' : 'descending');
            const key = button.dataset.sort;
            rows.sort((a, b) => direction * (key === 'iterations'
                ? Number(a.dataset[key]) - Number(b.dataset[key])
                : a.dataset[key].localeCompare(b.dataset[key])));
            document.querySelector('#records').append(...rows);
        });
    });
    form.addEventListener('submit', async event => {
        event.preventDefault();
        if (run.disabled) return;
        pendingJob = undefined;
        running = true;
        update();

        selectTab('output');
        frame.hidden = true;
        output.hidden = false;
        output.textContent = `Running ${form.elements.analysis.value}…`;
        try {
            const response = await fetch('/analyze', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    ...form.dataset,
                    analysis: form.elements.analysis.value,
                    records: selected().map(row => row.querySelector('input').value),
                }),
            });
            const text = await response.text();
            if (!response.ok) throw new Error(text);
            const job = JSON.parse(text);
            pendingJob = job.id;
            output.textContent = `${job.request.analysis}: ${job.state}…`;
            await pollJobs();
        } catch (error) {
            output.textContent = `Could not start analysis: ${error.message}`;
        } finally {
            running = false;
            update();
        }
    });
    filter();
}

const jobsPanel = document.querySelector('#jobs');
let jobSnapshot;
async function pollJobs() {
    if (!jobsPanel) return;
    try {
        const response = await fetch('/jobs', { cache: 'no-store' });
        if (!response.ok) throw new Error(await response.text());
        const snapshot = await response.text();
        const jobs = JSON.parse(snapshot);
        const pending = jobs.find(job => job.id === pendingJob);
        if (pending?.result) {
            viewOutput('output');
            frame.src = pending.result;
            pendingJob = undefined;
        } else if (pending?.error) {
            selectTab('output');
            frame.hidden = true;
            output.hidden = false;
            output.textContent = `Analysis failed: ${pending.error}`;
            pendingJob = undefined;
        }
        if (snapshot === jobSnapshot) return;
        jobSnapshot = snapshot;
        setBusy('jobs', jobs.some(job => ['queued', 'running'].includes(job.state)));
        const items = jobs.map(job => {
            const item = document.createElement('div');
            item.className = 'job';
            const label = document.createElement('p');
            label.textContent = job.request.analysis;
            label.title = `${job.request.project}/${job.request.fossil}`;
            const status = document.createElement('p');
            status.textContent = `${job.state} · ${job.progress.completed}/${job.progress.total} records`;
            item.append(label, status);
            if (job.error) {
                const error = document.createElement('p');
                error.textContent = job.error;
                item.append(error);
            }
            if (job.result) {
                const link = document.createElement('a');
                link.href = job.result;
                link.textContent = 'View result';
                link.target = frame ? 'output' : '_blank';
                item.append(link);
            }
            return item;
        });
        jobsPanel.replaceChildren(...items);
        if (!jobs.length) jobsPanel.textContent = 'No jobs yet.';
    } catch (error) {
        jobSnapshot = undefined;
        jobsPanel.textContent = `Job status unavailable: ${error.message}. Retrying…`;
    }
}
async function watchJobs() {
    await pollJobs();
    if (jobsPanel) setTimeout(watchJobs, 1500);
}
watchJobs();
