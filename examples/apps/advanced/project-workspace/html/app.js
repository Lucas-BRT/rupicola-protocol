'use strict';
const $ = (selector) => document.querySelector(selector);
const defaults = [{id:'1',name:'Website refresh',status:'In progress',progress:65},{id:'2',name:'Customer interviews',status:'Planned',progress:10},{id:'3',name:'Release checklist',status:'Completed',progress:100}];
let data;
try {
  const saved = JSON.parse(localStorage.getItem('project-workspace') || 'null');
  data = saved && Array.isArray(saved.projects) ? saved : {projects: defaults, name:'Project workspace', compact:false, dark:false};
} catch { data = {projects:defaults, name:'Project workspace', compact:false, dark:false}; }
let editing = null;
let opener = null;
const dialog = $('#editor');
function save() {
  try { localStorage.setItem('project-workspace', JSON.stringify(data)); return true; }
  catch { alert('Could not save to this browser. Changes may be lost.'); return false; }
}
function element(tag, text) { const node = document.createElement(tag); node.textContent = text; return node; }
function page(name) {
  for (const section of document.querySelectorAll('main > section')) section.hidden = section.id !== name;
  for (const button of document.querySelectorAll('nav button')) {
    if (button.dataset.page === name) button.setAttribute('aria-current','page'); else button.removeAttribute('aria-current');
  }
}
function render() {
  $('header h1').textContent = data.name || 'Project workspace';
  $('#workspace-name').value = data.name || '';
  $('#compact').checked = !!data.compact;
  document.documentElement.classList.toggle('dark', !!data.dark);
  $('#theme').textContent = data.dark ? 'Light theme' : 'Dark theme';
  $('#theme').setAttribute('aria-pressed', String(!!data.dark));
  $('#project-list').classList.toggle('compact', !!data.compact);
  $('#total').textContent = data.projects.length;
  $('#active').textContent = data.projects.filter(p => p.status === 'In progress').length;
  $('#completed').textContent = data.projects.filter(p => p.status === 'Completed').length;
  const recent = $('#recent'); recent.replaceChildren();
  for (const project of data.projects.slice(-5).reverse()) recent.append(element('li', `${project.name} — ${project.status}`));
  if (!data.projects.length) recent.append(element('li', 'No projects yet. Add a project.'));
  const term = $('#search').value.trim().toLocaleLowerCase();
  const status = $('#filter').value;
  const projects = data.projects.filter(p => p.name.toLocaleLowerCase().includes(term) && (status === 'all' || p.status === status));
  $('#result-count').textContent = `${projects.length} of ${data.projects.length} projects`;
  $('#empty').hidden = projects.length > 0;
  const list = $('#project-list'); list.replaceChildren();
  for (const project of projects) {
    const row = document.createElement('tr');
    row.append(element('td', project.name), element('td', project.status));
    const progress = document.createElement('td');
    const bar = document.createElement('progress'); bar.max = 100; bar.value = project.progress; bar.setAttribute('aria-label', `${project.name} progress`);
    progress.append(bar, document.createTextNode(` ${project.progress}%`)); row.append(progress);
    const actions = document.createElement('td');
    const edit = element('button', 'Edit'); edit.type = 'button'; edit.setAttribute('aria-label', `Edit ${project.name}`); edit.addEventListener('click', () => openEditor(project, edit));
    const remove = element('button', 'Delete'); remove.type = 'button'; remove.setAttribute('aria-label', `Delete ${project.name}`);
    remove.addEventListener('click', () => { if (confirm(`Delete ${project.name}?`)) { data.projects = data.projects.filter(p => p.id !== project.id); save(); render(); $('#new-project').focus(); } });
    actions.append(edit, remove); row.append(actions); list.append(row);
  }
}
function openEditor(project, button) {
  opener = button; editing = project?.id ?? null;
  $('#dialog-title').textContent = project ? 'Edit project' : 'Add project';
  $('#project-name').value = project?.name ?? '';
  $('#project-status').value = project?.status ?? 'Planned';
  $('#progress').value = project?.progress ?? 0;
  $('#progress-value').value = `${$('#progress').value}%`;
  dialog.showModal(); $('#project-name').focus();
}
for (const button of document.querySelectorAll('nav button')) button.addEventListener('click', () => page(button.dataset.page));
$('#new-project').addEventListener('click', event => openEditor(null, event.currentTarget));
$('#close-dialog').addEventListener('click', () => dialog.close());
$('#cancel').addEventListener('click', () => dialog.close());
dialog.addEventListener('close', () => opener?.focus());
$('#progress').addEventListener('input', event => { $('#progress-value').value = `${event.target.value}%`; });
$('#project-form').addEventListener('submit', event => {
  event.preventDefault();
  const name = $('#project-name').value.trim();
  if (!name) { $('#project-name').setCustomValidity('Enter a project name.'); $('#project-name').reportValidity(); return; }
  const project = {id:editing ?? crypto.randomUUID(), name, status:$('#project-status').value, progress:Number($('#progress').value)};
  if (editing) data.projects = data.projects.map(p => p.id === editing ? project : p); else data.projects.push(project);
  save(); dialog.close(); render(); $('#new-project').focus();
});
$('#project-name').addEventListener('input', event => event.target.setCustomValidity(''));
$('#search').addEventListener('input', render); $('#filter').addEventListener('change', render);
$('#theme').addEventListener('click', () => { data.dark = !data.dark; save(); render(); });
$('#settings-form').addEventListener('submit', event => {
  event.preventDefault(); const name = $('#workspace-name').value.trim();
  if (!name) { $('#workspace-name').setCustomValidity('Enter a workspace name.'); $('#workspace-name').reportValidity(); return; }
  data.name = name; data.compact = $('#compact').checked; const saved = save(); render(); $('#settings-message').textContent = saved ? 'Settings saved.' : 'Settings could not be saved.';
});
$('#workspace-name').addEventListener('input', event => event.target.setCustomValidity(''));
render();
