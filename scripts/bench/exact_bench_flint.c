/* Exact-arithmetic comparison suite: FLINT in C.
 *
 *   cc -O2 scripts/bench/exact_bench_flint.c -o exact_bench_flint -lflint -lgmp
 *   ./exact_bench_flint NAME N REPEATS
 *
 * Computes one case of scripts/bench/exact-bench-cases.json with FLINT's own
 * types, folding left to right as Ajisai's FOLD does, and prints one JSON
 * object: the best time of REPEATS runs and the answer, rendered the way
 * Ajisai displays it (`p/q`, `[ a b ]`) so scripts/bench/exact-bench.mjs can
 * compare by text. Rationals use fmpq. `sqrt-sum` uses Calcium's `ca`, FLINT's
 * exact real and complex numbers; `ca` has no square-free sum-of-radicals
 * normal form to print, so that answer is 40 significant digits from
 * `ca_get_decimal_str`, which the runner compares with Ajisai's `FORMAT` over
 * the leading fractional digits both carry.
 *
 * Tested with FLINT 3.0.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <flint/flint.h>
#include <flint/fmpq.h>
#include <flint/fmpq_vec.h>
#include <flint/ca.h>

static double now_ms(void)
{
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return ts.tv_sec * 1000.0 + ts.tv_nsec / 1.0e6;
}

/* Appends `p/q` to *out (realloc'd), q written even when it is 1. */
static void append_fmpq(char **out, size_t *len, const fmpq_t x)
{
    char *p = fmpz_get_str(NULL, 10, fmpq_numref(x));
    char *q = fmpz_get_str(NULL, 10, fmpq_denref(x));
    size_t add = strlen(p) + strlen(q) + 2;
    *out = realloc(*out, *len + add + 1);
    *len += (size_t) sprintf(*out + *len, "%s/%s", p, q);
    flint_free(p);
    flint_free(q);
}

static void append_text(char **out, size_t *len, const char *text)
{
    size_t add = strlen(text);
    *out = realloc(*out, *len + add + 1);
    memcpy(*out + *len, text, add + 1);
    *len += add;
}

/* Each case times its own arithmetic into *ms and renders the answer after
 * the clock stops, as the other systems are timed without their rendering. */

/* harmonic and basel: the sum of 1/k^power for k = 1..n. */
static char *reciprocal_sum(slong n, int power, double *ms)
{
    double started = now_ms();
    fmpq_t sum, term;
    fmpq_init(sum);
    fmpq_init(term);
    for (slong k = 1; k <= n; k++) {
        fmpq_set_si(term, 1, (ulong) (power == 1 ? k : k * k));
        fmpq_add(sum, sum, term);
    }
    *ms = now_ms() - started;
    char *out = NULL;
    size_t len = 0;
    append_fmpq(&out, &len, sum);
    fmpq_clear(sum);
    fmpq_clear(term);
    return out;
}

/* n Newton steps x <- (x + 2/x)/2 from x = 1. */
static char *newton_sqrt2(slong n, double *ms)
{
    double started = now_ms();
    fmpq_t x, t;
    fmpq_init(x);
    fmpq_init(t);
    fmpq_one(x);
    for (slong i = 0; i < n; i++) {
        fmpq_set_si(t, 2, 1);
        fmpq_div(t, t, x);
        fmpq_add(x, x, t);
        fmpq_div_2exp(x, x, 1);
    }
    *ms = now_ms() - started;
    char *out = NULL;
    size_t len = 0;
    append_fmpq(&out, &len, x);
    fmpq_clear(x);
    fmpq_clear(t);
    return out;
}

/* The vector of (k+1)/k * k/(k+2) for k = 1..n. */
static char *elementwise(slong n, double *ms)
{
    double started = now_ms();
    fmpq *v = _fmpq_vec_init(n);
    fmpq_t a, b;
    fmpq_init(a);
    fmpq_init(b);
    for (slong k = 1; k <= n; k++) {
        fmpq_set_si(a, k + 1, (ulong) k);
        fmpq_set_si(b, k, (ulong) (k + 2));
        fmpq_mul(v + (k - 1), a, b);
    }
    *ms = now_ms() - started;
    char *out = NULL;
    size_t len = 0;
    append_text(&out, &len, "[");
    for (slong i = 0; i < n; i++) {
        append_text(&out, &len, " ");
        append_fmpq(&out, &len, v + i);
    }
    append_text(&out, &len, " ]");
    _fmpq_vec_clear(v, n);
    fmpq_clear(a);
    fmpq_clear(b);
    return out;
}

/* sqrt(1) + ... + sqrt(n) in Calcium. */
static char *sqrt_sum(slong n, double *ms)
{
    double started = now_ms();
    ca_ctx_t ctx;
    ca_t sum, term;
    ca_ctx_init(ctx);
    ca_init(sum, ctx);
    ca_init(term, ctx);
    for (slong k = 1; k <= n; k++) {
        ca_sqrt_ui(term, (ulong) k, ctx);
        ca_add(sum, sum, term, ctx);
    }
    *ms = now_ms() - started;
    char *digits = ca_get_decimal_str(sum, 40, 0, ctx);
    char *out = strdup(digits);
    flint_free(digits);
    ca_clear(sum, ctx);
    ca_clear(term, ctx);
    ca_ctx_clear(ctx);
    return out;
}

static char *run(const char *name, slong n, double *ms)
{
    if (strcmp(name, "harmonic") == 0) return reciprocal_sum(n, 1, ms);
    if (strcmp(name, "basel") == 0) return reciprocal_sum(n, 2, ms);
    if (strcmp(name, "newton-sqrt2") == 0) return newton_sqrt2(n, ms);
    if (strcmp(name, "elementwise") == 0) return elementwise(n, ms);
    if (strcmp(name, "sqrt-sum") == 0) return sqrt_sum(n, ms);
    return NULL;
}

int main(int argc, char **argv)
{
    if (argc != 4) {
        fprintf(stderr, "usage: %s NAME N REPEATS\n", argv[0]);
        return 2;
    }
    const char *name = argv[1];
    slong n = atol(argv[2]);
    int repeats = atoi(argv[3]);
    double best = 1e300;
    char *answer = NULL;
    for (int r = 0; r < repeats; r++) {
        free(answer);
        double ms = 0;
        answer = run(name, n, &ms);
        if (answer == NULL) {
            fprintf(stderr, "unknown case %s\n", name);
            return 2;
        }
        if (ms < best) best = ms;
    }
    printf("{\"ms\":%.4f,\"version\":\"%s\",\"answer\":\"%s\"}\n", best, flint_version, answer);
    free(answer);
    flint_cleanup();
    return 0;
}
