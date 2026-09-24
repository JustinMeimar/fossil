const busy = new Set();
function setBusy(task, active) {
    active ? busy.add(task) : busy.delete(task);
    const indicator = document.querySelector('#busy');
    if (indicator) indicator.hidden = busy.size === 0;
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
let outputVersion = 0;
function viewOutput(target) {
    if (!frame) return;
    selectTab(target);
    setBusy(target, true);
    if (target !== 'output') return;
    outputVersion++;
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
        const version = ++outputVersion;
        running = true;
        update();
        setBusy('analysis', true);
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
            if (version === outputVersion) output.textContent = text;
        } catch (error) {
            if (version === outputVersion) output.textContent = `Analysis failed: ${error.message}`;
        } finally {
            running = false;
            setBusy('analysis', false);
            update();
        }
    });
    filter();
}
