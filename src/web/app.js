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

const expandViewer = document.querySelector('#expand-viewer');
expandViewer?.addEventListener('click', () => {
    const expanded = document.querySelector('.workspace').classList.toggle('viewer-focused');
    expandViewer.setAttribute('aria-pressed', String(expanded));
    expandViewer.textContent = expanded ? 'Show records' : 'Expand view';
});

const artifactFiles = document.querySelector('#artifact-file');
function showArtifact(url) {
    if (!url) return;
    artifactFiles.value = url;
    const selected = artifactFiles.selectedOptions[0];
    const generation = document.querySelector('#generate');
    if (generation && selected) {
        generation.elements.artifact.value = selected.dataset.artifact;
        updateGeneration();
    }
    const open = document.querySelector('#open-artifact');
    open.href = url;
    open.hidden = false;
    viewOutput('artifacts', url);
    document.querySelector('iframe[name=artifacts]').src = url;
}
function preferredArtifact(options) {
    return options.find(option => /\.pdf$/i.test(option.dataset.file)) || options[0];
}
artifactFiles?.addEventListener('change', () => showArtifact(artifactFiles.value));

const tabs = [...document.querySelectorAll('[role=tab]')];
function selectTab(id) {
    if (id === 'artifacts' && artifactFiles && !artifactFiles.value) {
        const preferred = preferredArtifact([...artifactFiles.options].filter(option => option.value));
        if (preferred) showArtifact(preferred.value);
    }
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
let knownJobs = [];
let analysisResult;
const generation = document.querySelector('#generate');
let generating = false;
function updateGeneration() {
    if (!generation) return;
    const artifact = generation.elements.artifact;
    const required = artifact.selectedOptions[0].dataset.analysis;
    const requiredVariants = JSON.parse(artifact.selectedOptions[0].dataset.requiredVariants || '[]');
    const matches = analysisResult && analysisResult.request.analysis === required
        && analysisResult.request.project === generation.dataset.project
        && analysisResult.request.fossil === generation.dataset.fossil;
    const available = new Set(matches ? analysisResult.variants : []);
    const missing = requiredVariants.filter(variant => !available.has(variant));
    generation.querySelector('button').disabled = generating || !artifact.value || (required && !matches) || missing.length > 0;
    if (generating) return;
    let message = '';
    if (artifact.value && required && !matches) {
        message = `View a completed ${required} analysis for this fossil first.`;
        if (requiredVariants.length) message += ` Required variants: ${requiredVariants.join(', ')}.`;
    } else if (artifact.value && missing.length) {
        message = `Missing required variants: ${missing.join(', ')}. Select records for these variants and rerun ${required}.`;
    } else if (artifact.value && required) {
        message = `Uses the ${required} result shown in Output.`;
    }
    document.querySelector('#generation-status').textContent = message;
}
function viewOutput(target, url) {
    if (!frame) return;
    selectTab(target);
    setBusy(target, true);
    document.querySelector(`iframe[name=${target}]`).hidden = false;
    if (target !== 'output') return;
    analysisResult = knownJobs.find(job => job.result === url);
    updateGeneration();
    output.hidden = true;
}

document.addEventListener('click', event => {
    const link = event.target.closest('a');
    if (!link || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) return;
    if (['output', 'artifacts'].includes(link.target)) {
        (window === window.parent ? window : window.parent).viewOutput(link.target, link.getAttribute('href'));
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
        analysisResult = undefined;
        updateGeneration();
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

if (generation) {
    const status = document.querySelector('#generation-status');
    generation.addEventListener('change', updateGeneration);
    generation.addEventListener('submit', async event => {
        event.preventDefault();
        if (generation.querySelector('button').disabled) return;
        const artifact = generation.elements.artifact.value;
        const request = { ...generation.dataset, artifact, job: analysisResult?.id };
        generating = true;
        updateGeneration();
        status.textContent = `Generating ${artifact}…`;
        setBusy('generation', true);
        try {
            const response = await fetch('/generate', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(request),
            });
            if (!response.ok) throw new Error(await response.text());
            const files = await response.json();
            [...artifactFiles.querySelectorAll('optgroup')]
                .filter(group => group.dataset.artifact === artifact)
                .forEach(group => group.remove());
            const group = document.createElement('optgroup');
            group.label = artifact;
            group.dataset.artifact = artifact;
            const options = files.map(file => {
                const option = document.createElement('option');
                option.value = `/output?${new URLSearchParams({ ...generation.dataset, artifact, file })}`;
                option.dataset.artifact = artifact;
                option.dataset.file = file;
                option.textContent = file;
                return option;
            });
            group.append(...options);
            artifactFiles.append(group);
            artifactFiles.disabled = ![...artifactFiles.options].some(option => option.value);
            document.querySelector('#artifact-empty').hidden = !artifactFiles.disabled;
            const preferred = preferredArtifact(options);
            if (preferred) showArtifact(preferred.value);
            status.textContent = `Generated ${artifact}.`;
        } catch (error) {
            status.textContent = `Could not generate ${artifact}: ${error.message}`;
        } finally {
            const message = status.textContent;
            generating = false;
            updateGeneration();
            status.textContent = message;
            setBusy('generation', false);
        }
    });
    updateGeneration();
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
        knownJobs = jobs;
        const pending = jobs.find(job => job.id === pendingJob);
        if (pending?.result) {
            viewOutput('output', pending.result);
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

for (const raw of document.querySelectorAll('pre[data-json]')) {
    let value;
    try { value = JSON.parse(raw.textContent); } catch { continue; }
    const tree = document.createElement('div');
    tree.className = 'json-tree';
    tree.setAttribute('aria-label', 'JSON preview');
    const toolbar = document.createElement('div');
    toolbar.className = 'json-toolbar';
    function button(label, action) {
        const control = document.createElement('button');
        control.type = 'button';
        control.textContent = label;
        control.addEventListener('click', action);
        toolbar.append(control);
        return control;
    }
    function labeled(label, content) {
        const key = document.createElement('span');
        key.className = 'json-key';
        key.textContent = `${label}: `;
        content.append(key);
    }
    function node(label, item, depth = 0) {
        if (item === null || typeof item !== 'object') {
            const leaf = document.createElement('div');
            leaf.className = 'json-leaf';
            labeled(label, leaf);
            const scalar = document.createElement('span');
            scalar.className = `json-${item === null ? 'null' : typeof item}`;
            scalar.textContent = JSON.stringify(item);
            leaf.append(scalar);
            return leaf;
        }
        const entries = Object.entries(item);
        const details = document.createElement('details');
        const summary = document.createElement('summary');
        labeled(label, summary);
        const hint = document.createElement('span');
        hint.className = 'json-kind';
        hint.textContent = Array.isArray(item) ? `[${entries.length} items]` : `{${entries.length} fields}`;
        summary.append(hint);
        const children = document.createElement('div');
        children.className = 'json-children';
        let offset = 0;
        const more = document.createElement('button');
        more.type = 'button';
        more.className = 'json-more';
        function appendPage() {
            const end = Math.min(offset + 100, entries.length);
            for (; offset < end; offset++) {
                const [key, child] = entries[offset];
                children.append(node(key, child, depth + 1));
            }
            if (offset < entries.length) {
                more.textContent = `Show next ${Math.min(100, entries.length - offset)} (${entries.length - offset} remaining)`;
                children.append(more);
            } else {
                more.remove();
            }
        }
        more.addEventListener('click', () => {
            const next = offset;
            appendPage();
            const first = children.children[next];
            const focus = first?.querySelector('summary') || first;
            if (focus) {
                if (focus.tagName !== 'SUMMARY') focus.tabIndex = -1;
                focus.focus();
            }
        });
        details.addEventListener('toggle', () => {
            if (details.open && offset === 0) appendPage();
        });
        details.append(summary, children);
        details.open = depth < 2;
        return details;
    }
    const expand = button('Expand one level', () => {
        const closed = [...tree.querySelectorAll('details:not([open])')]
            .filter(details => !details.parentElement.closest('details:not([open])'));
        closed.forEach(details => details.open = true);
    });
    const collapse = button('Collapse all', () => {
        tree.querySelectorAll('details').forEach(details => details.open = false);
    });
    const toggle = button('Show raw JSON', () => {
        const showRaw = raw.hidden;
        raw.hidden = !showRaw;
        tree.hidden = showRaw;
        expand.disabled = collapse.disabled = showRaw;
        toggle.textContent = showRaw ? 'Show tree' : 'Show raw JSON';
        toggle.setAttribute('aria-pressed', String(showRaw));
    });
    toggle.setAttribute('aria-pressed', 'false');
    tree.append(node('JSON', value));
    raw.before(toolbar, tree);
    raw.hidden = true;
}
