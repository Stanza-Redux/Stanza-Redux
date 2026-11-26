// SPDX-License-Identifier: GPL-2.0-or-later
//! What day-build does before this project compiles: today, the typed `res::` constants
//! generated from `resource/` (https://daybrite.dev/docs/resources).
fn main() {
    day_build::prebuild_project().expect("day-build: prebuild");
}
