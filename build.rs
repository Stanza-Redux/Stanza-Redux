// SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception
//! What day-build does before this project compiles: today, the typed `res::` constants
//! generated from `resource/` (https://daybrite.dev/docs/resources).
fn main() {
    day_build::prebuild_project().expect("day-build: prebuild");
}
