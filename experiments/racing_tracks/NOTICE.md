# Data attribution

The source circuit centerlines are derived from
[`bacinger/f1-circuits`](https://github.com/bacinger/f1-circuits), revision
`394d8fbe70ef2c0b0c8d23ff7bee61fa09606055`, licensed under the MIT License.

Copyright (c) 2019–2025 Tomislav Bacinger.

The Spa elevation prototype uses EU-DEM v1.1 through OpenTopoData. EU-DEM is a
Copernicus product made available under the Copernicus full, open and free
access policy. The derived prototype has been adapted and smoothed by Pitgun
and is not endorsed by the European Union, Copernicus, or OpenTopoData.

- Dataset documentation: https://www.opentopodata.org/datasets/eudem/
- API documentation: https://www.opentopodata.org/api/

The independent Spa validation uses the `Relief de la Wallonie - Modèle
Numérique de Terrain (MNT) 2021-2022`, a 0.5 metre LiDAR-derived terrain model
published by the Service public de Wallonie under CC BY 4.0. Pitgun converts
the existing centerline to WGS84, asks the official REST service to transform
and identify each terrain height, then applies its own documented smoothing
and comparison. The source data has therefore been adapted by Pitgun.

Required source citation:

> Service public de Wallonie (SPW) - Relief de la Wallonie - Modèle Numérique
> de Terrain (MNT) 2021-2022 (2024-01-23)

- Dataset: https://geodata.wallonie.be/id/a004e570-99d6-4fe5-b83d-49b774409278
- REST service: https://geoservices.wallonie.be/arcgis/rest/services/RELIEF/WALLONIE_MNT_2021_2022/MapServer
