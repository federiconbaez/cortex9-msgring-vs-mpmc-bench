# Análisis

Corridas: 170 (inválidas: 0). Bootstrap: 10000 remuestreos, semilla base 7, IC 95 %.

Ruido: steal total 7 ticks; loadavg 1 min por corrida entre 4.85 y 8.41.

## Medias por celda (latencia en µs, media de los valores por corrida)

| modo | P | n | p50 | p99 | p99.9 | msg/s (throughput) |
|---|---|---|---|---|---|---|
| msgring | 1 | 10 | 3.9 | 3081.3 | 4582.6 | 1522027 |
| msgring | 7 | 10 | 3.3 | 5193.4 | 6604.8 | 2439556 |
| msgring_defer | 1 | 10 | 4.0 | 2069.0 | 3780.1 | 1746018 |
| msgring_defer | 7 | 10 | 4.1 | 5970.3 | 7957.1 | 3415988 |
| mpmc_eventfd | 1 | 10 | 4.1 | 3659.1 | 5344.5 | 1726716 |
| mpmc_eventfd | 7 | 10 | 3.8 | 4435.4 | 6017.3 | 2316484 |
| mpmc_spin | 1 | 10 | 0.2 | 1427.7 | 1938.7 | 18795719 |
| mpmc_spin | 7 | 10 | 133.9 | 4660.4 | 7190.2 | 4020586 |

## Controles

- Negativo A/A (mpmc_eventfd P=1, p99 main − aa): 45.2 µs, IC [-1335.3, 1457.6] → PASA (incluye 0)
- Positivo (mpmc_eventfd p99 P=7 − P=1): 776.3 µs, IC [-656.2, 2279.5] → FALLA (sin sensibilidad)

## Hipótesis primaria (p99, msgring − mpmc_eventfd)

- P=1: -577.8 µs, IC [-1494.3, 280.7] → no se declara: falló el control positivo
- P=7: 758.0 µs, IC [-1299.5, 2677.1] → no se declara: falló el control positivo

## Exploratorio (todos los pares; sin corrección por comparaciones múltiples)

| métrica | P | A | B | A − B | IC 95 % | veredicto |
|---|---|---|---|---|---|---|
| p50_ns | 1 | msgring | msgring_defer | -0.1 µs | [-0.5, 0.3] | IC cruza 0 |
| p50_ns | 1 | msgring | mpmc_eventfd | -0.2 µs | [-0.5, 0.1] | IC cruza 0 |
| p50_ns | 1 | msgring | mpmc_spin | 3.7 µs | [3.4, 3.9] | menor: mpmc_spin |
| p50_ns | 1 | msgring_defer | mpmc_eventfd | -0.1 µs | [-0.4, 0.2] | IC cruza 0 |
| p50_ns | 1 | msgring_defer | mpmc_spin | 3.8 µs | [3.5, 4.1] | menor: mpmc_spin |
| p50_ns | 1 | mpmc_eventfd | mpmc_spin | 3.9 µs | [3.7, 4.0] | menor: mpmc_spin |
| p50_ns | 7 | msgring | msgring_defer | -0.9 µs | [-1.3, -0.4] | menor: msgring |
| p50_ns | 7 | msgring | mpmc_eventfd | -0.5 µs | [-0.9, -0.2] | menor: msgring |
| p50_ns | 7 | msgring | mpmc_spin | -130.6 µs | [-349.1, 2.6] | IC cruza 0 |
| p50_ns | 7 | msgring_defer | mpmc_eventfd | 0.3 µs | [-0.0, 0.7] | IC cruza 0 |
| p50_ns | 7 | msgring_defer | mpmc_spin | -129.8 µs | [-349.2, 3.4] | IC cruza 0 |
| p50_ns | 7 | mpmc_eventfd | mpmc_spin | -130.1 µs | [-348.4, 3.0] | IC cruza 0 |
| p99_ns | 1 | msgring | msgring_defer | 1012.3 µs | [-470.9, 2448.4] | IC cruza 0 |
| p99_ns | 1 | msgring | mpmc_eventfd | -577.8 µs | [-1537.3, 299.3] | IC cruza 0 |
| p99_ns | 1 | msgring | mpmc_spin | 1653.6 µs | [-253.9, 3205.1] | IC cruza 0 |
| p99_ns | 1 | msgring_defer | mpmc_eventfd | -1590.1 µs | [-2934.3, -245.2] | menor: msgring_defer |
| p99_ns | 1 | msgring_defer | mpmc_spin | 641.3 µs | [-1493.1, 2496.1] | IC cruza 0 |
| p99_ns | 1 | mpmc_eventfd | mpmc_spin | 2231.4 µs | [407.6, 3668.7] | menor: mpmc_spin |
| p99_ns | 7 | msgring | msgring_defer | -776.9 µs | [-4595.6, 2197.9] | IC cruza 0 |
| p99_ns | 7 | msgring | mpmc_eventfd | 758.0 µs | [-1279.4, 2677.9] | IC cruza 0 |
| p99_ns | 7 | msgring | mpmc_spin | 533.0 µs | [-1711.7, 2795.0] | IC cruza 0 |
| p99_ns | 7 | msgring_defer | mpmc_eventfd | 1534.9 µs | [-1459.9, 5379.7] | IC cruza 0 |
| p99_ns | 7 | msgring_defer | mpmc_spin | 1309.9 µs | [-1902.1, 5269.9] | IC cruza 0 |
| p99_ns | 7 | mpmc_eventfd | mpmc_spin | -225.0 µs | [-2387.2, 1974.6] | IC cruza 0 |
| p999_ns | 1 | msgring | msgring_defer | 802.4 µs | [-1177.1, 2680.0] | IC cruza 0 |
| p999_ns | 1 | msgring | mpmc_eventfd | -761.9 µs | [-1842.3, 231.5] | IC cruza 0 |
| p999_ns | 1 | msgring | mpmc_spin | 2643.8 µs | [780.5, 4258.2] | menor: mpmc_spin |
| p999_ns | 1 | msgring_defer | mpmc_eventfd | -1564.3 µs | [-3521.2, 459.8] | IC cruza 0 |
| p999_ns | 1 | msgring_defer | mpmc_spin | 1841.4 µs | [-677.2, 4235.0] | IC cruza 0 |
| p999_ns | 1 | mpmc_eventfd | mpmc_spin | 3405.8 µs | [1553.3, 5197.0] | menor: mpmc_spin |
| p999_ns | 7 | msgring | msgring_defer | -1352.3 µs | [-5709.1, 2070.7] | IC cruza 0 |
| p999_ns | 7 | msgring | mpmc_eventfd | 587.5 µs | [-1400.2, 2500.2] | IC cruza 0 |
| p999_ns | 7 | msgring | mpmc_spin | -585.4 µs | [-2930.7, 1696.7] | IC cruza 0 |
| p999_ns | 7 | msgring_defer | mpmc_eventfd | 1939.7 µs | [-1307.4, 6129.5] | IC cruza 0 |
| p999_ns | 7 | msgring_defer | mpmc_spin | 766.9 µs | [-2860.9, 5151.0] | IC cruza 0 |
| p999_ns | 7 | mpmc_eventfd | mpmc_spin | -1172.8 µs | [-3331.6, 1034.2] | IC cruza 0 |
| msgs_per_s | 1 | msgring | msgring_defer | -223991.2 msg/s | [-619132.4, 219388.1] | IC cruza 0 |
| msgs_per_s | 1 | msgring | mpmc_eventfd | -204688.7 msg/s | [-670695.1, 300352.8] | IC cruza 0 |
| msgs_per_s | 1 | msgring | mpmc_spin | -17273692.4 msg/s | [-22867754.1, -10920450.2] | mayor: mpmc_spin |
| msgs_per_s | 1 | msgring_defer | mpmc_eventfd | 19302.5 msg/s | [-464167.3, 515644.9] | IC cruza 0 |
| msgs_per_s | 1 | msgring_defer | mpmc_spin | -17049701.2 msg/s | [-22606266.1, -10713009.2] | mayor: mpmc_spin |
| msgs_per_s | 1 | mpmc_eventfd | mpmc_spin | -17069003.7 msg/s | [-22642219.6, -10770212.6] | mayor: mpmc_spin |
| msgs_per_s | 7 | msgring | msgring_defer | -976432.2 msg/s | [-1207190.0, -736800.6] | mayor: msgring_defer |
| msgs_per_s | 7 | msgring | mpmc_eventfd | 123071.3 msg/s | [-387527.0, 563304.1] | IC cruza 0 |
| msgs_per_s | 7 | msgring | mpmc_spin | -1581030.8 msg/s | [-2276263.4, -923247.8] | mayor: mpmc_spin |
| msgs_per_s | 7 | msgring_defer | mpmc_eventfd | 1099503.5 msg/s | [560755.2, 1587431.1] | mayor: msgring_defer |
| msgs_per_s | 7 | msgring_defer | mpmc_spin | -604598.6 msg/s | [-1327265.3, 86181.8] | IC cruza 0 |
| msgs_per_s | 7 | mpmc_eventfd | mpmc_spin | -1704102.1 msg/s | [-2542978.6, -889112.4] | mayor: mpmc_spin |
