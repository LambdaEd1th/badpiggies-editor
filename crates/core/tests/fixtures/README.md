# Bad Piggies test fixtures

These unmodified game files are separated by game version so parser tests do
not accidentally treat the newer level format as v1.5.1 data.

## v1.5.1

Source: `../BadPiggies_v1.5.1/BadPiggies_Data/` relative to the `editor`
workspace.

- `v1.5.1/assets/resources.assets`: complete Unity 4.2.1 resources file with
  the v1.5.1 level TextAsset catalog.
- `v1.5.1/assets/sharedassets8.assets`: minimal Unity 4.2.1 SerializedFile.
- `v1.5.1/assets/sharedassets17.assets`: multi-object Unity 4.2.1
  SerializedFile.
- `v1.5.1/levels/Level_05_data.bytes` and `scenario_58_data.bytes`: unchanged
  TextAsset payloads extracted from the v1.5.1 `resources.assets` fixture.

## v2.3.6

Sources relative to the `editor` workspace:

- Level bytes: `../test_levels/assetbundles/`
- UnityFS bundles: `../backup/AssetBundles/`

- `v2.3.6/levels/Level_05_data.bytes`: representative v2.3.6 binary level.
- `v2.3.6/levels/scenario_58_data.bytes`: larger v2.3.6 scenario with complex
  terrain and background references.
- `v2.3.6/bundles/Episode_1_Levels.unity3d`: chapter bundle used for listing,
  reading, and in-memory replacement tests.
- `v2.3.6/bundles/Episode_Race_Levels.unity3d`: bundle containing all eight
  race levels.

The same-name `.bytes` fixtures are intentionally retained in both version
directories because their serialized contents differ.

All fixtures are read-only inputs. Tests write modified bundles only to the
system temporary directory.

`CHECKSUMS.sha256` records the exact fixture contents.
