// Copyright 2025 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later

import SwiftUI

struct BrowseView: View {
    var body: some View {
        NavigationStack {
            VStack(spacing: 20) {
                Image("explore", bundle: .module)
                    .font(.system(size: 64))
                    .foregroundStyle(.secondary)
                Text("Browse book catalogs to discover and download new books.")
                    .multilineTextAlignment(.center)
                    .foregroundStyle(.secondary)
                    .padding(.horizontal)
            }
            .navigationTitle("Browse")
        }
    }
}
