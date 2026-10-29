# cortex9-msgring-vs-mpmc-bench

[Español](#español) · [English](#english)

## Español

Benchmark **pre-registrado** de paso de mensajes de N productores a 1
consumidor: `IORING_OP_MSG_RING` (con y sin `DEFER_TASKRUN`) contra la cola
lock-free `federiconbaez/lockfree-mpmc` (commit `c4d8fd0`) con eventfd o con
busy-poll. Reporta p50, p99, p99.9 y msg/s con IC 95 % por bootstrap.

### Pre-registro

El diseño (`DESIGN.md`, `design.json`: modos, cargas, P ∈ {1, 7}, 10
repeticiones, semillas, análisis, hipótesis, umbral y controles) se
commiteó en `dc3b06b` y se **publicó en GitHub antes de escribir el harness y
antes de medir** (push del 2026-10-29 03:07:39 UTC, 00:07:39 ART). Los sha256
están en `PREREGISTRO.sha256` y un test verifica que no cambiaron. El
binario lee `design.json`, así que lo medido es lo pre-registrado.

### Resultado (lo que dice el pre-registro)

**No se declara ningún ganador.** El control positivo falló: con
`mpmc_eventfd`, el p99 con 7 productores contra 1 dio +776 µs con IC 95 %
[−656, +2280] µs, que cruza 0. Según el diseño, si el control positivo
falla el harness no tiene sensibilidad en esta máquina y la hipótesis
primaria no se evalúa. El control negativo A/A pasó (diferencia de p99 de
+45 µs, IC [−1335, +1458] µs).

| Hipótesis primaria: p99 msgring − mpmc_eventfd | Diferencia | IC 95 % | Veredicto |
|---|---|---|---|
| P=1 | −578 µs | [−1494, +281] | no se declara |
| P=7 | +758 µs | [−1300, +2677] | no se declara |

Medias por celda (media de los valores por corrida, 10 corridas):

| modo | P | p50 µs | p99 µs | p99.9 µs | msg/s |
|---|---|---|---|---|---|
| msgring | 1 | 3,9 | 3081 | 4583 | 1,52 M |
| msgring | 7 | 3,3 | 5193 | 6605 | 2,44 M |
| msgring_defer | 1 | 4,0 | 2069 | 3780 | 1,75 M |
| msgring_defer | 7 | 4,1 | 5970 | 7957 | 3,42 M |
| mpmc_eventfd | 1 | 4,1 | 3659 | 5345 | 1,73 M |
| mpmc_eventfd | 7 | 3,8 | 4435 | 6017 | 2,32 M |
| mpmc_spin | 1 | 0,2 | 1428 | 1939 | 18,8 M |
| mpmc_spin | 7 | 133,9 | 4660 | 7190 | 4,02 M |

Exploratorio (sin corrección por comparaciones múltiples; ver
`results/analysis.md`): el p50 de `mpmc_spin` con P=1 es menor que el de
los otros tres modos (IC excluye 0); en throughput `mpmc_spin` es mayor en
casi todos los pares y `msgring_defer` supera a `msgring` y a
`mpmc_eventfd` con P=7. Las colas (p99 y p99.9) están en milisegundos en
todos los modos: lo más probable es que reflejen desalojo de hilos en una
VM sobrecargada y no el mecanismo de paso de mensajes.

### Máquina y ruido (leer antes de usar los números)

- VM **compartida y ruidosa** de 8 vCPU Intel Xeon, kernel **6.12.94+**,
  sin aislamiento de núcleos y con otros procesos corriendo. Loadavg de 1
  minuto entre 4,85 y 8,41 durante las 170 corridas; steal total 7 ticks.
- Con P=7 hay 8 hilos fijados en 8 vCPU ya cargadas: hay sobresuscripción.
- Medición del 2026-10-29 entre 03:11:18 y 03:12:10 UTC (00:11 a 00:12 ART) con el harness `4bfca22`.
- **El resultado no se generaliza** a otra máquina, otro kernel ni a un
  motor real. Para una conclusión hay que repetirlo en una máquina con
  núcleos aislados.
- Los números de latencia del CI (smoke) no son la medición.

### Reproducir

```
cargo build --release
./target/release/msgring-bench run design.json results/runs.csv
./target/release/msgring-bench analyze design.json results/runs.csv
```

`analyze` sale con código 3 cuando falla un control (como acá).

## English

A **pre-registered** benchmark of passing messages from N producers to one
consumer: `IORING_OP_MSG_RING` (with and without `DEFER_TASKRUN`) versus the
lock-free queue `federiconbaez/lockfree-mpmc` (commit `c4d8fd0`) with an
eventfd or busy-polling. It reports p50/p99/p99.9 and msg/s with bootstrap
95 % CIs.

The design was committed in `dc3b06b` and **pushed to GitHub before the
harness existed and before measuring** (2026-10-29 03:07:39 UTC).

**Result: no winner is declared.** The pre-registered positive control
failed (mpmc_eventfd p99, 7 vs 1 producers: +776 µs, 95 % CI [−656, +2280]
µs), so per the design the primary hypothesis (msgring vs mpmc_eventfd p99
at P=1 and P=7) is not evaluated; both CIs also cross 0 (P=1: −578 µs
[−1494, +281]; P=7: +758 µs [−1300, +2677]). The A/A control passed.
Exploratory: busy-polling has the lowest p50 at P=1 and the highest
throughput in most pairs; msgring_defer beats msgring and mpmc_eventfd on
throughput at P=7. Millisecond tails in every mode most likely reflect
thread preemption on an overloaded VM.

Measured on a **shared, noisy** 8-vCPU VM, kernel 6.12.94+, load average
4.85–8.41, no core isolation. **Do not generalize** these numbers. CI
latency numbers are a smoke test, not the measurement.

## Licencia / License

Apache-2.0.
