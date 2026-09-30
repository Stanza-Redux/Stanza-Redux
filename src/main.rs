// SPDX-License-Identifier: AGPL-3.0-only WITH App-Fair-Distribution-Exception

fn main() {
    // The same window and root the mobile hosts open through `day_start!` in src/lib.rs.
    day::launch(dayapp::window(), dayapp::root);
}
