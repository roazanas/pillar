#include <ctype.h>
#include <errno.h>
#include <inttypes.h>
#include <limits.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#ifdef _WIN32
#include <fcntl.h>
#include <io.h>
#include <windows.h>
#include <shellapi.h>
#endif

typedef struct Obj Obj;
typedef struct {
  uint64_t bits;
  int64_t tag;
} Cell;
struct Obj {
  Obj *prev, *next;
  uint64_t refs;
  int64_t len, kind;
  unsigned char data[];
};
static Obj *objects;
static const char *file = "<program>";
static int64_t line = 1, column = 1;
static int depth;
static int program_argc;
static char **program_argv;
static void fatal(const char *code, const char *message) {
  fprintf(stderr, "%s:%" PRId64 ":%" PRId64 ": error[%s]: %s\n", file, line,
          column, code, message);
  while (objects) {
    Obj *next = objects->next;
    free(objects);
    objects = next;
  }
  exit(1);
}
void plr_location(const char *path, int64_t row, int64_t col) {
  file = path;
  line = row;
  column = col;
}
void plr_init(int64_t argc, char **argv) {
  program_argc = (int)argc;
  program_argv = argv;
#ifdef _WIN32
  _setmode(_fileno(stdin), _O_BINARY);
  _setmode(_fileno(stdout), _O_BINARY);
  _setmode(_fileno(stderr), _O_BINARY);
#endif
}
void plr_enter(void) {
  if (++depth > 256)
    fatal("R0012", "function call depth limit exceeded");
}
void plr_leave(void) { depth--; }

int32_t plr_finish(int64_t status) {
  if (objects)
    fatal("R0099", "runtime reference ownership invariant failed");
  if (status < 0 || status > 255)
    fatal("R0008", "main exit code must be in 0..255");
  return (int32_t)status;
}
static Obj *obj(uint64_t h) { return (Obj *)(uintptr_t)h; }
static uint64_t handle(Obj *o) { return (uint64_t)(uintptr_t)o; }
static int reference(int64_t tag) { return tag >= 3 && tag <= 5; }
static Obj *allocate(int64_t kind, int64_t len) {
  if (len < 0 || len > INT64_MAX / 32)
    fatal("R0009", "allocation size overflow");
  size_t bytes = kind == 3 ? (size_t)len + 1 : (size_t)len * sizeof(Cell);
  if (bytes > SIZE_MAX - sizeof(Obj))
    fatal("R0009", "allocation size overflow");
  Obj *o = (Obj *)calloc(1, sizeof(Obj) + bytes);
  if (!o)
    fatal("R0009", "out of memory");
  o->refs = 1;
  o->len = len;
  o->kind = kind;
  o->next = objects;
  if (objects)
    objects->prev = o;
  objects = o;
  return o;
}
uint64_t plr_retain(uint64_t h) {
  if (h) {
    Obj *o = obj(h);
    if (o->refs == UINT64_MAX)
      fatal("R0009", "reference count overflow");
    o->refs++;
  }
  return h;
}
void plr_release(uint64_t h) {
  if (!h)
    return;
  Obj *o = obj(h);
  if (--o->refs)
    return;
  if (o->prev)
    o->prev->next = o->next;
  else
    objects = o->next;
  if (o->next)
    o->next->prev = o->prev;
  if (o->kind != 3) {
    Cell *cells = (Cell *)o->data;
    for (int64_t i = 0; i < o->len; i++)
      if (reference(cells[i].tag))
        plr_release(cells[i].bits);
  }
  free(o);
}
static int valid_utf8(const unsigned char *s, size_t n) {
  for (size_t i = 0; i < n;) {
    unsigned c = s[i++];
    if (c < 128)
      continue;
    unsigned need, cp, min;
    if (c >= 0xc2 && c <= 0xdf) {
      need = 1;
      cp = c & 31;
      min = 128;
    } else if (c >= 0xe0 && c <= 0xef) {
      need = 2;
      cp = c & 15;
      min = 2048;
    } else if (c >= 0xf0 && c <= 0xf4) {
      need = 3;
      cp = c & 7;
      min = 65536;
    } else
      return 0;
    if (i + need > n)
      return 0;
    while (need--) {
      unsigned b = s[i++];
      if ((b & 0xc0) != 0x80)
        return 0;
      cp = (cp << 6) | (b & 63);
    }
    if (cp < min || cp > 0x10ffff || (cp >= 0xd800 && cp <= 0xdfff))
      return 0;
  }
  return 1;
}
uint64_t plr_string(const char *data, int64_t len) {
  if (len < 0 || !valid_utf8((const unsigned char *)data, (size_t)len))
    fatal("R0007", "input is not valid UTF-8");
  Obj *o = allocate(3, len);
  memcpy(o->data, data, (size_t)len);
  return handle(o);
}
uint64_t plr_collection(int64_t len, int64_t kind) {
  return handle(allocate(kind, len));
}
static void bounds(Obj *o, int64_t index) {
  if (index < 0 || index >= o->len)
    fatal("R0004", "array index out of bounds");
}
uint64_t plr_get(uint64_t h, int64_t index) {
  Obj *o = obj(h);
  bounds(o, index);
  Cell c = ((Cell *)o->data)[index];
  if (reference(c.tag))
    plr_retain(c.bits);
  plr_release(h);
  return c.bits;
}
uint64_t plr_set(uint64_t h, int64_t index, uint64_t bits, int64_t tag) {
  Obj *o = obj(h);
  bounds(o, index);
  if (o->refs > 1) {
    Obj *copy = allocate(o->kind, o->len);
    memcpy(copy->data, o->data, (size_t)o->len * sizeof(Cell));
    Cell *cells = (Cell *)copy->data;
    for (int64_t i = 0; i < o->len; i++)
      if (reference(cells[i].tag))
        plr_retain(cells[i].bits);
    plr_release(h);
    o = copy;
  }
  Cell *c = &((Cell *)o->data)[index];
  if (reference(c->tag))
    plr_release(c->bits);
  c->bits = bits;
  c->tag = tag;
  return handle(o);
}
uint64_t plr_push(uint64_t h, uint64_t bits, int64_t tag) {
  Obj *o = obj(h), *n = allocate(4, o->len + 1);
  Cell *old = (Cell *)o->data, *cells = (Cell *)n->data;
  for (int64_t i = 0; i < o->len; i++) {
    cells[i] = old[i];
    if (reference(cells[i].tag))
      plr_retain(cells[i].bits);
  }
  cells[o->len] = (Cell){bits, tag};
  plr_release(h);
  return handle(n);
}
uint64_t plr_concat(uint64_t a, uint64_t b) {
  Obj *x = obj(a), *y = obj(b);
  if (x->len > INT64_MAX - y->len)
    fatal("R0009", "string length overflow");
  Obj *n = allocate(3, x->len + y->len);
  memcpy(n->data, x->data, (size_t)x->len);
  memcpy(n->data + x->len, y->data, (size_t)y->len);
  plr_release(a);
  plr_release(b);
  return handle(n);
}
int64_t plr_string_equal(uint64_t a, uint64_t b) {
  Obj *x = obj(a), *y = obj(b);
  int64_t result =
      x->len == y->len && !memcmp(x->data, y->data, (size_t)x->len);
  plr_release(a);
  plr_release(b);
  return result;
}
int64_t plr_len(uint64_t h, int64_t bytes) {
  Obj *o = obj(h);
  int64_t n = o->len;
  if (o->kind == 3 && !bytes) {
    n = 0;
    for (int64_t i = 0; i < o->len; i++)
      if ((o->data[i] & 0xc0) != 0x80)
        n++;
  }
  plr_release(h);
  return n;
}
static double as_float(uint64_t bits) {
  double f;
  memcpy(&f, &bits, 8);
  return f;
}
static uint64_t float_bits(double f) {
  uint64_t bits;
  memcpy(&bits, &f, 8);
  return bits;
}
void plr_print(uint64_t bits, int64_t tag, int64_t newline) {
  if (tag == 0)
    printf("%" PRId64, (int64_t)bits);
  else if (tag == 1)
    printf("%g", as_float(bits));
  else if (tag == 2)
    fputs(bits ? "true" : "false", stdout);
  else {
    Obj *o = obj(bits);
    fwrite(o->data, 1, (size_t)o->len, stdout);
    plr_release(bits);
  }
  if (newline)
    fputc('\n', stdout);
}
int64_t plr_add(int64_t a, int64_t b) {
  if ((b > 0 && a > INT64_MAX - b) || (b < 0 && a < INT64_MIN - b))
    fatal("R0001", "integer addition overflow");
  return a + b;
}
int64_t plr_sub(int64_t a, int64_t b) {
  if ((b < 0 && a > INT64_MAX + b) || (b > 0 && a < INT64_MIN + b))
    fatal("R0001", "integer subtraction overflow");
  return a - b;
}
int64_t plr_mul(int64_t a, int64_t b) {
  if ((a > 0 &&
       ((b > 0 && a > INT64_MAX / b) || (b < 0 && b < INT64_MIN / a))) ||
      (a < 0 && ((b > 0 && a < INT64_MIN / b) || (b < 0 && a < INT64_MAX / b))))
    fatal("R0001", "integer multiplication overflow");
  return a * b;
}
int64_t plr_neg(int64_t a) {
  if (a == INT64_MIN)
    fatal("R0001", "integer negation overflow");
  return -a;
}
int64_t plr_div(int64_t a, int64_t b) {
  if (!b)
    fatal("R0002", "division by zero");
  if (a == INT64_MIN && b == -1)
    fatal("R0001", "integer division overflow");
  return a / b;
}
uint64_t plr_div_float(uint64_t a, uint64_t b) {
  double divisor = as_float(b);
  if (divisor == 0.0)
    fatal("R0002", "division by zero");
  return float_bits(as_float(a) / divisor);
}
int64_t plr_mod(int64_t a, int64_t b) {
  if (!b)
    fatal("R0002", "remainder by zero");
  if (a == INT64_MIN && b == -1)
    fatal("R0001", "integer remainder overflow");
  return a % b;
}
int64_t plr_abs_int(int64_t a) { return a < 0 ? plr_neg(a) : a; }
void plr_assert(int64_t condition) {
  if (!condition)
    fatal("R0005", "assertion failed");
}
static void no_nul(Obj *s) {
  if (memchr(s->data, 0, (size_t)s->len))
    fatal("R0006", "embedded NUL is not allowed here");
}
int64_t plr_int(uint64_t bits, int64_t tag) {
  if (tag == 0)
    return (int64_t)bits;
  if (tag == 1) {
    double f = as_float(bits);
    if (!isfinite(f) || f < -9223372036854775808.0 ||
        f >= 9223372036854775808.0)
      fatal("R0003", "float is outside int range");
    return (int64_t)f;
  }
  Obj *s = obj(bits);
  no_nul(s);
  errno = 0;
  char *end;
  intmax_t value = strtoimax((char *)s->data, &end, 10);
  int converted = end != (char *)s->data;
  while (isspace((unsigned char)*end))
    end++;
  if (errno == ERANGE || !converted || *end || value < INT64_MIN ||
      value > INT64_MAX)
    fatal("R0003", "invalid integer input");
  plr_release(bits);
  return (int64_t)value;
}
uint64_t plr_float(uint64_t bits, int64_t tag) {
  if (tag == 1)
    return bits;
  if (tag == 0)
    return float_bits((double)(int64_t)bits);
  Obj *s = obj(bits);
  no_nul(s);
  errno = 0;
  char *end;
  double value = strtod((char *)s->data, &end);
  int converted = end != (char *)s->data;
  while (isspace((unsigned char)*end))
    end++;
  if (errno == ERANGE || !converted || *end || !isfinite(value))
    fatal("R0003", "invalid float input");
  plr_release(bits);
  return float_bits(value);
}
uint64_t plr_str(uint64_t bits, int64_t tag) {
  if (tag == 3)
    return bits;
  char buffer[128];
  int n;
  if (tag == 0)
    n = snprintf(buffer, sizeof(buffer), "%" PRId64, (int64_t)bits);
  else if (tag == 1)
    n = snprintf(buffer, sizeof(buffer), "%g", as_float(bits));
  else
    n = snprintf(buffer, sizeof(buffer), "%s", bits ? "true" : "false");
  return plr_string(buffer, n);
}
static uint64_t read_input(int token) {
  size_t len = 0, cap = 128;
  char *data = (char *)malloc(cap);
  if (!data)
    fatal("R0009", "out of memory");
  int c = fgetc(stdin);
  if (token)
    while (c != EOF && isspace((unsigned char)c))
      c = fgetc(stdin);
  if (c == EOF) {
    free(data);
    fatal("R0010", ferror(stdin) ? "input read failed" : "end of input");
  }
  while (c != EOF && (token ? !isspace((unsigned char)c) : c != '\n')) {
    if (len == cap) {
      if (cap > SIZE_MAX / 2)
        fatal("R0009", "input too large");
      cap *= 2;
      char *p = (char *)realloc(data, cap);
      if (!p) {
        free(data);
        fatal("R0009", "out of memory");
      }
      data = p;
    }
    data[len++] = (char)c;
    c = fgetc(stdin);
  }
  if (token && c == '\r') {
    int next = fgetc(stdin);
    if (next != '\n' && next != EOF)
      ungetc(next, stdin);
  }
  if (ferror(stdin)) {
    free(data);
    fatal("R0010", "input read failed");
  }
  if (!token && len && data[len - 1] == '\r')
    len--;
  if (!valid_utf8((const unsigned char *)data, len)) {
    free(data);
    fatal("R0007", "input is not valid UTF-8");
  }
  uint64_t result = plr_string(data, (int64_t)len);
  free(data);
  return result;
}
int64_t plr_read_int(void) { return plr_int(read_input(1), 3); }
uint64_t plr_read_float(void) { return plr_float(read_input(1), 3); }
uint64_t plr_read_line(void) { return read_input(0); }
uint64_t plr_args(void) {
#ifdef _WIN32
  int count = 0;
  wchar_t **wide = CommandLineToArgvW(GetCommandLineW(), &count);
  if (!wide)
    fatal("R0007", "cannot decode program arguments");
  Obj *result = allocate(4, count > 0 ? count - 1 : 0);
  Cell *items = (Cell *)result->data;
  for (int i = 1; i < count; i++) {
    int n = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, wide[i], -1,
                                NULL, 0, NULL, NULL);
    if (!n)
      fatal("R0007", "invalid Unicode argument");
    Obj *s = allocate(3, n - 1);
    WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, wide[i], -1,
                        (char *)s->data, n, NULL, NULL);
    items[i - 1] = (Cell){handle(s), 3};
  }
  LocalFree(wide);
  return handle(result);
#else
  Obj *a = allocate(4, program_argc > 0 ? program_argc - 1 : 0);
  Cell *cells = (Cell *)a->data;
  for (int64_t i = 0; i < a->len; i++)
    cells[i] = (Cell){
        plr_string(program_argv[i + 1], (int64_t)strlen(program_argv[i + 1])),
        3};
  return handle(a);
#endif
}
uint64_t plr_sqrt(uint64_t bits) {
  double f = as_float(bits);
  if (f < 0)
    fatal("R0003", "sqrt requires a nonnegative value");
  return float_bits(sqrt(f));
}
uint64_t plr_pow(uint64_t a, uint64_t b) {
  return float_bits(pow(as_float(a), as_float(b)));
}
uint64_t plr_abs_float(uint64_t a) { return float_bits(fabs(as_float(a))); }
static FILE *open_path(Obj *s, const char *mode) {
  no_nul(s);
#ifdef _WIN32
  int n = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS,
                              (const char *)s->data, -1, NULL, 0);
  if (!n)
    fatal("R0011", "invalid file path");
  wchar_t *path = (wchar_t *)malloc((size_t)n * sizeof(wchar_t));
  if (!path)
    fatal("R0009", "out of memory");
  MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, (const char *)s->data, -1,
                      path, n);
  FILE *f = _wfopen(path, mode[0] == 'r' ? L"rb" : L"wb");
  free(path);
  return f;
#else
  return fopen((const char *)s->data, mode);
#endif
}
uint64_t plr_read_file(uint64_t path) {
  FILE *f = open_path(obj(path), "rb");
  plr_release(path);
  if (!f)
    fatal("R0011", "cannot open input file");
  size_t len = 0, cap = 4096;
  char *data = (char *)malloc(cap);
  if (!data)
    fatal("R0009", "out of memory");
  for (;;) {
    size_t n = fread(data + len, 1, cap - len, f);
    len += n;
    if (n == 0)
      break;
    if (len == cap) {
      if (cap > SIZE_MAX / 2)
        fatal("R0009", "file too large");
      cap *= 2;
      char *p = (char *)realloc(data, cap);
      if (!p) {
        free(data);
        fatal("R0009", "out of memory");
      }
      data = p;
    }
  }
  if (ferror(f)) {
    fclose(f);
    free(data);
    fatal("R0011", "file read failed");
  }
  fclose(f);
  if (!valid_utf8((const unsigned char *)data, len)) {
    free(data);
    fatal("R0007", "input is not valid UTF-8");
  }
  uint64_t result = plr_string(data, (int64_t)len);
  free(data);
  return result;
}
void plr_write_file(uint64_t path, uint64_t value) {
  FILE *f = open_path(obj(path), "wb");
  plr_release(path);
  if (!f)
    fatal("R0011", "cannot open output file");
  Obj *s = obj(value);
  size_t n = fwrite(s->data, 1, (size_t)s->len, f);
  int closed = fclose(f);
  if (n != (size_t)s->len || closed)
    fatal("R0011", "file write failed");
  plr_release(value);
}
