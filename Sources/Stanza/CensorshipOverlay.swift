// Copyright 2026 The App Fair Project
// SPDX-License-Identifier: GPL-2.0-or-later

import SwiftUI
import StanzaModel

/// Localized "Censored by …" text describing the storefront that imposed the restriction.
func censorshipBannerText(for storefront: String) -> Text {
    switch storefront {
    case Storefront.googlePlayStore:
        return Text("Censored by Google", bundle: .module, comment: "censorship banner shown over restricted book covers on Google Play")
    case Storefront.appleAppStore:
        return Text("Censored by Apple", bundle: .module, comment: "censorship banner shown over restricted book covers on the App Store")
    default:
        return Text("Censored", bundle: .module, comment: "fallback censorship banner shown over restricted book covers")
    }
}

extension View {
    /// Applies a blur and censorship overlay to a book cover when the book uid has a `cover` mode restriction
    /// for the running app's storefront. Returns the view unchanged when there is no restriction.
    @ViewBuilder
    func contentRestrictedCover(uid: String?) -> some View {
        if let uid = uid, let restriction = ContentRestrictionService.shared.restriction(forUID: uid), restriction.mode == .cover {
            self
                .blur(radius: 8)
                .overlay {
                    censorshipBannerText(for: Storefront.current)
                        .font(.caption2)
                        .fontWeight(.semibold)
                        .multilineTextAlignment(.center)
                        .foregroundStyle(.white)
                        .padding(.horizontal, 4)
                        .padding(.vertical, 3)
                        .background(Color.black.opacity(0.7))
                        .accessibilityIdentifier("censoredCoverBanner")
                }
        } else {
            self
        }
    }
}
