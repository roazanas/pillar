'use strict';
const $ = id => document.getElementById(id);
const source = $('source');
const examples = {
  hello: 'FN main(): int\n{\n  println("Hello, Pillar!")~\n  RET 0~\n}\n',
  array: 'FN main(): int\n{\n  LT a: [int] = [1, 2, 3]~\n  LT b: [int] = a~\n  b[0] = 9~\n  OUTI a[0]~\n  OUTI b[0]~\n  RET 0~\n}\n',
  input: 'FN main(): int\n{\n  println("Enter a number:")~\n  LT n: int = read_int()~\n  OUTI n * 2~\n  RET 0~\n}\n'
};
function clearError() { $('source-error').textContent = ''; source.removeAttribute('aria-invalid'); }
function update() {
  try { $('preview').value = PillarColumns.toHorizontal(source.value); }
  catch (error) { $('source-error').textContent = error.message; }
}
const history = [], future = [];
let lastState;
function remember() { if (lastState && lastState.text !== source.value) { history.push(lastState); future.length = 0; } lastState = { text: source.value, offset: source.selectionStart }; }
function apply(result) { clearError(); if (lastState) { history.push({ text: source.value, offset: source.selectionStart }); future.length = 0; } source.value = result.text; source.setSelectionRange(result.offset, result.offset); source.focus(); lastState = { text: source.value, offset: source.selectionStart }; update(); }
source.addEventListener('input', () => { clearError(); remember(); update(); });
source.addEventListener('select', () => { if (lastState) lastState.offset = source.selectionStart; });
source.addEventListener('beforeinput', event => {
  if (!$('downward').checked || event.isComposing || source.selectionStart !== source.selectionEnd) return;
  if (event.inputType === 'insertLineBreak') { event.preventDefault(); apply(PillarColumns.nextColumn(source.value, source.selectionStart)); }
  else if (event.inputType === 'insertText' && event.data) {
    event.preventDefault(); apply(PillarColumns.editDown(source.value, source.selectionStart, event.data));
  } else if (event.inputType === 'deleteContentForward') {
    event.preventDefault(); apply(PillarColumns.editDown(source.value, source.selectionStart, '', 1));
  } else if (event.inputType === 'deleteContentBackward') {
    const { row, col } = PillarColumns.caret(source.value, source.selectionStart);
    event.preventDefault();
    if (row > 0) {
      const lines = source.value.split('\n');
      const at = lines.slice(0, row - 1).reduce((n, line) => n + line.length + 1, 0) + Array.from(lines[row - 1]).slice(0, col).join('').length;
      apply(PillarColumns.editDown(source.value, at, '', 1));
    }
  }
});
source.addEventListener('paste', event => {
  if (!$('downward').checked || source.selectionStart !== source.selectionEnd) return;
  event.preventDefault(); apply(PillarColumns.editDown(source.value, source.selectionStart, event.clipboardData.getData('text/plain')));
});
for (const action of ['insert', 'remove', 'left', 'right']) $(action).addEventListener('click', () => apply(PillarColumns.column(source.value, source.selectionStart, action)));
for (const [id, from, to] of [['undo', history, future], ['redo', future, history]]) $(id).addEventListener('click', () => {
  const state = from.pop(); if (!state) return;
  to.push({ text: source.value, offset: source.selectionStart });
  source.value = state.text; source.setSelectionRange(state.offset, state.offset); lastState = state; source.focus(); update();
});
source.addEventListener('keydown', event => { if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'z') { event.preventDefault(); $(event.shiftKey ? 'redo' : 'undo').click(); } });
let composing;
source.addEventListener('compositionstart', () => { if ($('downward').checked && source.selectionStart === source.selectionEnd) composing = { text: source.value, offset: source.selectionStart }; });
source.addEventListener('compositionend', event => { if (!composing) return; const before = composing; composing = undefined; queueMicrotask(() => { source.value = before.text; source.setSelectionRange(before.offset, before.offset); lastState = before; apply(PillarColumns.editDown(before.text, before.offset, event.data)); }); });
$('example').addEventListener('change', () => { source.value = PillarColumns.toVertical(examples[$('example').value]); $('source-error').textContent = ''; source.removeAttribute('aria-invalid'); remember(); update(); });
$('import').addEventListener('click', () => { document.querySelector('details').open = true; $('horizontal').focus(); });
$('apply-import').addEventListener('click', () => { try { source.value = PillarColumns.toVertical($('horizontal').value); clearError(); remember(); update(); source.focus(); $('status').textContent = 'Horizontal source imported.'; } catch (error) { $('source-error').textContent = error.message; } });
$('download').addEventListener('click', () => { const url = URL.createObjectURL(new Blob([source.value], { type: 'text/plain;charset=utf-8' })); const link = document.createElement('a'); link.href = url; link.download = 'main.plr'; link.click(); setTimeout(() => URL.revokeObjectURL(url), 1000); });
async function request(action) {
  const buttons = ['run', 'check', 'format'].map($);
  buttons.forEach(button => { button.disabled = true; });
  $('status').textContent = action === 'run' ? 'Running program…' : 'Checking source…';
  $('source-error').textContent = ''; source.removeAttribute('aria-invalid');
  try {
    const response = await fetch('/api', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ action, source: source.value, stdin: $('stdin').value }) });
    const result = await response.json();
    if (!response.ok) throw new Error(result.error || 'Unable to complete the request. Try again.');
    if (action === 'fmt' && result.status === 0) { source.value = result.stdout; remember(); update(); $('output').textContent = 'Source formatted.'; }
    else { $('output').textContent = result.stdout || ''; }
    if (result.stderr) {
      let message = result.stderr;
      try { const diagnostic = JSON.parse(result.stderr); message = diagnostic.message; if (diagnostic.labels?.length) message += ` (row ${diagnostic.labels[0].start.line}, column ${diagnostic.labels[0].start.column})`; } catch {}
      if (!['run', 'sandbox'].includes(result.phase)) { $('source-error').textContent = message; source.setAttribute('aria-invalid', 'true'); }
      $('output').textContent += (result.stdout ? '\n' : '') + message;
    }
    $('status').textContent = result.timed_out ? 'Stopped at the time limit.' : result.truncated ? `Finished with status ${result.status}; output was truncated.` : `Finished with status ${result.status}.`;
    if (!result.stdout && !result.stderr && action === 'check') $('output').textContent = 'No errors found.';
  } catch (error) { $('output').textContent = error.message; $('status').textContent = 'Request could not be completed.'; }
  finally { buttons.forEach(button => { button.disabled = false; }); }
}
$('run').addEventListener('click', () => request('run'));
$('check').addEventListener('click', () => request('check'));
$('format').addEventListener('click', () => request('fmt'));
source.value = PillarColumns.toVertical(examples.hello);
lastState = { text: source.value, offset: 0 };
update();
