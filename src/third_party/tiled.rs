//! Tiled maps as Bevy assets, parsed with the `tiled` crate (rs-tiled). A
//! `.tmx` file under `assets/` loads into a [`TiledMap`]: its external
//! tilesets (`.tsx`) are read through the asset server and tracked as
//! dependencies, and so is each tileset's image, so the map only reports
//! loaded once everything it needs is ready — and reloads when any of them
//! changes. `tiled` types stay behind the asset: game plugins only read what
//! [`TiledMap`] and the parsed map expose.

use std::{
    cell::RefCell,
    collections::HashMap,
    io::{self, Cursor},
    path::{Component, Path, PathBuf},
};

use bevy::{
    asset::{AssetLoader, AssetPath, LoadContext, io::Reader},
    prelude::*,
    tasks::ConditionalSendFuture,
};
use tiled::{Loader, Orientation, StaggerAxis, StaggerIndex};

/// A Tiled map (`.tmx`) loaded through the asset server, with the image of
/// each of its tilesets.
///
/// Only maps laid out like the game's hex grid are accepted: finite
/// "Hexagonal (Staggered)" maps with the x stagger axis and an odd stagger
/// index, whose tilesets each have a single image.
#[derive(Asset, TypePath)]
pub struct TiledMap {
    map: tiled::Map,
    /// Image of each tileset of [`TiledMap::map`], in the same order.
    tileset_images: Vec<Handle<Image>>,
}

impl TiledMap {
    /// The parsed map.
    pub fn map(&self) -> &tiled::Map {
        &self.map
    }

    /// The image holding the tiles of the tileset at `index` in
    /// `self.map().tilesets()`, where each tile is a cell of the tileset's
    /// grid.
    pub fn tileset_image(&self, index: usize) -> &Handle<Image> {
        &self.tileset_images[index]
    }
}

/// Loads `.tmx` files as [`TiledMap`] assets.
#[derive(Default, TypePath)]
pub struct TiledMapLoader;

impl AssetLoader for TiledMapLoader {
    type Asset = TiledMap;
    type Settings = ();
    type Error = tiled::Error;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _: &(),
        context: &mut LoadContext<'_>,
    ) -> Result<TiledMap, tiled::Error> {
        let map_path = context.path().path().to_path_buf();

        let mut bytes = Vec::new();
        reader
            .read_to_end(&mut bytes)
            .await
            .map_err(|err| loading_error(&map_path, err))?;
        let mut sources = HashMap::from([(map_path.clone(), bytes)]);

        // `tiled` reads external tilesets through a synchronous reader while
        // the asset server is asynchronous: parse, note every file the parse
        // reached for, fetch the ones not in hand, and parse again. Every
        // round loads at least one more file or returns, so this ends.
        let map = loop {
            let (result, requested) = parse_map(&map_path, &sources);
            let missing: Vec<PathBuf> = requested
                .iter()
                .filter(|path| !sources.contains_key(*path))
                .cloned()
                .collect();
            if missing.is_empty() {
                break result?;
            }
            for path in missing {
                let bytes = context
                    .read_asset_bytes(asset_path(&path))
                    .await
                    .map_err(|err| loading_error(&path, err))?;
                sources.insert(path, bytes);
            }
        };
        check_layout(&map)?;

        let tileset_images = map
            .tilesets()
            .iter()
            .map(|tileset| {
                let image = tileset.image.as_ref().ok_or_else(|| {
                    tiled::Error::MalformedAttributes(format!(
                        "tileset {} has no image; image collection tilesets are not supported",
                        tileset.name
                    ))
                })?;
                Ok(context.load::<Image>(asset_path(&image.source)))
            })
            .collect::<Result<_, tiled::Error>>()?;

        Ok(TiledMap {
            map,
            tileset_images,
        })
    }

    fn extensions(&self) -> &[&str] {
        &["tmx"]
    }
}

/// Parses the map at `map_path` out of `sources` (file path to contents), and
/// lists every file the parse reached for, whether or not it was in `sources`.
fn parse_map(
    map_path: &Path,
    sources: &HashMap<PathBuf, Vec<u8>>,
) -> (tiled::Result<tiled::Map>, Vec<PathBuf>) {
    let requested = RefCell::new(Vec::new());
    let read = |path: &Path| -> io::Result<Cursor<&[u8]>> {
        requested.borrow_mut().push(path.to_path_buf());
        sources
            .get(path)
            .map(|bytes| Cursor::new(bytes.as_slice()))
            .ok_or_else(|| io::ErrorKind::NotFound.into())
    };
    let result = Loader::with_reader(read).load_tmx_map(map_path);
    (result, requested.into_inner())
}

/// Rejects every map layout but the game's hex grid (`Tiling.md`): flat-top
/// hexagons with odd columns offset by half a hex, and finite so it has a
/// size and bounds.
fn check_layout(map: &tiled::Map) -> tiled::Result<()> {
    let supported = map.orientation == Orientation::Hexagonal
        && map.stagger_axis == StaggerAxis::X
        && map.stagger_index == StaggerIndex::Odd
        && !map.infinite();
    if supported {
        Ok(())
    } else {
        Err(tiled::Error::MalformedAttributes(format!(
            "{}: only finite hexagonal maps with the x stagger axis and an odd stagger index are supported",
            map.source.display()
        )))
    }
}

/// Turns a file reference from a map or tileset, which is relative to the
/// file naming it, into an asset path. `..` segments are collapsed here,
/// since the asset server refuses paths that leave its root through them.
fn asset_path(path: &Path) -> AssetPath<'static> {
    let mut resolved = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                resolved.pop();
            }
            Component::CurDir => {}
            component => resolved.push(component),
        }
    }
    AssetPath::from_path_buf(resolved)
}

/// Wraps a failure to read `path` as the error `tiled` reports for files it
/// cannot open.
fn loading_error(path: &Path, err: impl std::error::Error + Send + Sync + 'static) -> tiled::Error {
    tiled::Error::ResourceLoadingError {
        path: path.to_path_buf(),
        err: Box::new(err),
    }
}

pub(crate) fn plugin(app: &mut App) {
    app.init_asset::<TiledMap>()
        .init_asset_loader::<TiledMapLoader>();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Loads `tiled/Test.tmx` through a bare asset server and hands the
    /// parsed asset to `check` once it is in.
    fn with_test_map(check: impl FnOnce(&TiledMap)) {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()));
        plugin(&mut app);
        // The map's tileset images are loaded as dependencies; the type must
        // exist for that, decoding them is not needed here.
        app.init_asset::<Image>();

        let handle = app.world().resource::<AssetServer>().load("tiled/Test.tmx");
        // Frames are near-instant headless, so wait on the clock while the
        // asset server's threads do the actual loading.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while std::time::Instant::now() < deadline {
            app.update();
            if let Some(map) = app.world().resource::<Assets<TiledMap>>().get(&handle) {
                check(map);
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        panic!("tiled/Test.tmx did not load");
    }

    #[test]
    fn loads_the_map_with_its_external_tileset() {
        with_test_map(|asset| {
            let map = asset.map();
            let [tileset] = map.tilesets() else {
                panic!("expected a single tileset, found {}", map.tilesets().len());
            };
            assert_eq!(tileset.name, "Test");
            assert_eq!((tileset.tile_width, tileset.tile_height), (32, 32));
            // `Test.tsx` refers to its image from inside `assets/tiled`.
            assert_eq!(
                asset.tileset_image(0).path().map(AssetPath::path),
                Some(Path::new("sprites/tilemaps/test_tile.png"))
            );
        });
    }

    #[test]
    fn asset_paths_collapse_parent_segments() {
        assert_eq!(
            asset_path(Path::new("tiled/../sprites/tilemaps/test_tile.png")).path(),
            Path::new("sprites/tilemaps/test_tile.png")
        );
    }
}
