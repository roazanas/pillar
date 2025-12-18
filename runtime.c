#include <stdio.h>

long read_int(void) {
  long n;
  if (scanf("%ld", &n) != 1) {
    return 0;
  }
  return n;
}

double read_float(void) {
  double n;
  if (scanf("%lf", &n) != 1) {
    return 0.0;
  }
  return n;
}

void print_int(long n) { printf("%ld", n); }

void print_float(double n) { printf("%g", n); }

void print_int_ln(long n) { printf("%ld\n", n); }

void print_float_ln(double n) { printf("%g\n", n); }
