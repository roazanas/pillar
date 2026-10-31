'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const grid = require('./vscode/columns');
test('Unicode scalars round trip and CRLF', () => {
  const source = 'FN main(): int\n{\n  println("😀")~\n  RET 0~\n}\n';
  assert.deepEqual(grid.toHorizontal(grid.toVertical(source)).split('\n').filter(s => s.trim()).map(s => s.trimEnd()), source.split('\n').filter(s => s.trim()).map(s => s.trimEnd()));
  assert.equal(grid.toVertical(source), grid.toVertical(source.replaceAll('\n', '\r\n')));
});
test('insert and delete downward without shifting adjacent columns', () => {
  const original = 'a X\nb Y\nc Z';
  assert.equal(grid.editDown(original, 0, '😀').text, '😀 X\na Y\nb Z\nc');
  assert.equal(grid.editDown(original, 0, '', 1).text, 'b X\nc Y\n  Z');
});
test('column insertion, deletion and movement are reversible', () => {
  const text = 'ab\ncd';
  const added = grid.column(text, 0, 'insert');
  assert.equal(added.text, ' ab\n cd');
  assert.equal(grid.column(added.text, 0, 'remove').text, text);
  assert.equal(grid.column(text, 0, 'right').text, 'ba\ndc');
});
test('new statement preserves neighboring columns and leaves an editable caret', () => {
  const result = grid.nextColumn('ab\ncd', 0);
  assert.equal(result.text, 'a  b\nc  d');
  assert.equal(result.offset, 2);
  const last = grid.column('a\nb', 1, 'insert');
  assert.equal(last.offset, 1);
});
