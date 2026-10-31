/* Shared Unicode-scalar grid operations for the browser and VS Code. */
(function (root) {
  'use strict';
  const rows = text => text.replace(/\r\n?/g, '\n').split('\n').map(line => Array.from(line));
  function transpose(text, spacing = false) {
    const matrix = rows(text.replace(/\n$/, ''));
    const width = Math.max(0, ...matrix.map(row => row.length));
    if (matrix.length * width > 4194304) throw new Error('The source exceeds the grid limit.');
    return Array.from({ length: width }, (_, col) => matrix.map(row => row[col] || ' ').join(spacing ? ' ' : '').trimEnd()).join('\n') + '\n';
  }
  function caret(text, offset) {
    const prefix = text.slice(0, offset).replace(/\r\n?/g, '\n').split('\n');
    return { row: prefix.length - 1, col: Array.from(prefix.at(-1)).length };
  }
  function finish(matrix, row, col) {
    const lines = matrix.map(line => line.join('').trimEnd());
    row = Math.min(row, lines.length - 1);
    while (Array.from(lines[row]).length < col) lines[row] += ' ';
    const text = lines.join('\n');
    const offset = lines.slice(0, row).reduce((n, line) => n + line.length + 1, 0) + Array.from(lines[row]).slice(0, col).join('').length;
    return { text, offset };
  }
  function editDown(text, offset, insert, remove = 0) {
    const matrix = rows(text), { row, col } = caret(text, offset);
    const column = matrix.map(line => line[col] || ' ');
    const content = Array.from(insert.replace(/\r\n?/g, ' '));
    column.splice(row, remove, ...content);
    const height = Math.max(matrix.length, column.length);
    while (matrix.length < height) matrix.push([]);
    for (let r = 0; r < height; r++) {
      while (matrix[r].length <= col) matrix[r].push(' ');
      matrix[r][col] = column[r] || ' ';
    }
    return finish(matrix, Math.min(row + content.length, height - 1), col);
  }
  function column(text, offset, action) {
    const matrix = rows(text), { row, col } = caret(text, offset);
    const target = action === 'left' ? Math.max(0, col - 1) : col + 1;
    for (const line of matrix) {
      while (line.length <= Math.max(col, target)) line.push(' ');
      if (action === 'insert') line.splice(col, 0, ' ');
      else if (action === 'remove') line.splice(col, 1);
      else [line[col], line[target]] = [line[target], line[col]];
    }
    return finish(matrix, row, ['left', 'right'].includes(action) ? target : col);
  }
  function nextColumn(text, offset) {
    const matrix = rows(text), { col } = caret(text, offset);
    for (const line of matrix) {
      while (line.length <= col) line.push(' ');
      line.splice(col + 1, 0, ' ', ' ');
    }
    return finish(matrix, 0, col + 2);
  }
  const api = { nextColumn, toHorizontal: text => transpose(text), toVertical: text => transpose(text, true), caret, editDown, column };
  if (typeof module !== 'undefined') module.exports = api;
  else root.PillarColumns = api;
})(typeof globalThis !== 'undefined' ? globalThis : this);
