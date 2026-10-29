# Diseño pre-registrado / Pre-registered design

Este archivo y `design.json` se commitean y se publican en GitHub **antes**
de escribir el harness de medición y antes de medir. El binario lee
`design.json`, así que los parámetros medidos son estos.

This file and `design.json` are committed and pushed to GitHub **before**
the measurement harness exists and before any measurement.

## Pregunta

Para pasar mensajes de N productores a 1 consumidor en un motor
thread-per-core, ¿cuánto cambia la latencia extremo a extremo y el
throughput entre `IORING_OP_MSG_RING` y una cola MPMC lock-free
(`federiconbaez/lockfree-mpmc`, Vyukov acotada) con eventfd o con
busy-poll?

## Modos

| Modo | Productor | Consumidor |
|---|---|---|
| `msgring` | ring propio (64 entradas), una SQE `MSG_RING` (`MsgRingData`, `IOSQE_CQE_SKIP_SUCCESS`) y un `io_uring_enter` por mensaje | ring sin flags especiales, CQ de 32768; bloquea en `submit_and_wait(1)` y drena la CQ |
| `msgring_defer` | igual | ring con `SINGLE_ISSUER` + `DEFER_TASKRUN` creado en el hilo consumidor |
| `mpmc_eventfd` | `push` a la cola (capacidad 4096, reintenta si está llena) y `write(eventfd, 1)` por mensaje | drena la cola; si está vacía bloquea en `read(eventfd)` |
| `mpmc_spin` | `push` a la cola, sin syscall | `pop` en bucle activo |

Mensaje: timestamp en ns (reloj monotónico común) + id de productor +
secuencia. En `msgring` el timestamp va en `user_data` y productor/secuencia
en `res`.

Hilos fijados: consumidor en la CPU 0, productor i en la CPU i (1..7).

## Cargas

- **Latencia**: cada productor manda 10.000 mensajes, uno cada 50 µs (espera
  activa hasta el próximo deadline, 20.000 msg/s por productor). Se
  descartan los primeros 1.000 mensajes de cada productor. Latencia =
  recepción − timestamp del productor.
- **Throughput**: cada productor manda 50.000 mensajes sin pausa. msg/s =
  mensajes recibidos / (última recepción − primer envío).
- Productores: P ∈ {1, 7}.
- Repeticiones: 10 por celda (modo × P × carga), más 10 corridas extra de
  `mpmc_eventfd`, P=1, latencia, como lote A/A.
- Orden: todas las corridas se barajan con Fisher-Yates y splitmix64,
  semilla 20261029.

## Análisis

- Por corrida: p50, p99 y p99.9 por rango más cercano sobre las muestras
  ordenadas; msg/s en throughput.
- Por celda: media de los valores por corrida.
- Diferencias A − B: bootstrap de percentiles, 10.000 remuestreos de las
  corridas de cada celda por separado, semilla 7, IC 95 %.
- **Se declara ganador solo si el IC 95 % de la diferencia no cruza 0.**

## Hipótesis primaria (dos comparaciones, bilaterales)

H1: el p99 de `msgring` difiere del de `mpmc_eventfd`, con P=1 y con P=7.
Umbral: IC 95 % de la diferencia de medias de p99 por corrida que excluye 0.
No hay corrección por comparaciones múltiples; todo lo demás (otros pares,
p50, p99.9, msg/s, `msgring_defer`, `mpmc_spin`) es **exploratorio**.

## Controles

- **Negativo (A/A)**: `mpmc_eventfd` P=1, lote principal contra lote A/A. El
  IC de la diferencia de p99 tiene que incluir 0. Si lo excluye, el ruido de
  la máquina domina y las declaraciones primarias se reportan como no
  confiables.
- **Positivo**: `mpmc_eventfd`, p99 con P=7 contra P=1. El IC tiene que
  excluir 0. Si lo incluye, el harness no tiene sensibilidad y no se declara
  ningún ganador.

## Ruido y validez

- Máquina: VM compartida de 8 vCPU (Intel Xeon), kernel 6.12.94+, sin
  aislamiento de núcleos y con otros procesos corriendo. Por corrida se
  registra el delta de steal de `/proc/stat` y el loadavg de 1 minuto.
- Una corrida con un error de `MSG_RING`, un mensaje perdido o duplicado es
  inválida: se reporta y no se repite en silencio.
- El resultado no se generaliza fuera de esta máquina y este kernel.
