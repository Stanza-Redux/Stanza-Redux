app_title = Stanza Redux
language_name = Français
library = Bibliothèque
catalogs = Catalogues
reader = Lecture
settings = Réglages
error = Impossible de terminer :
loading = Chargement…
downloading = Téléchargement…
chapter = Chapitre
cancel = Annuler
library_title = Votre prochain chapitre
library_hint = Les livres téléchargés restent sur cet appareil, disponibles hors ligne.
read = Lire
delete = Supprimer
catalog_url = Adresse du catalogue OPDS
go = Ouvrir
save = Enregistrer le catalogue
back = Retour
search = Rechercher
details = Détails du livre
next_results = Résultats suivants
download = Télécharger l’EPUB
epub_only = Les livres EPUB non chiffrés sont pris en charge.
previous = Précédente
next = Suivante
reader_empty = Choisissez un livre dans votre bibliothèque ou téléchargez-en un depuis un catalogue.
font_size = Taille du texte
margins = Marges latérales
vertical_margin = Marge verticale
line_height = Interligne
theme = Thème
light = Clair
sepia = Sépia
dark = Sombre
font = Police
columns = Colonnes
columns_hint = Deux colonnes sur les fenêtres larges ; une seule sur les écrans étroits.
import = Importer un EPUB
too_large = Ce livre dépasse la limite de 64 Mo.
serif = Avec empattements
sans = Sans empattements
mono = Chasse fixe
font_montserrat = Montserrat
font_noto_sans = Noto Sans
font_noto_serif = Noto Serif
custom = Personnel
paper_color = Couleur de papier personnalisée
ink_color = Couleur de texte personnalisée
custom_hint = Choisissez le thème Personnel pour utiliser ces couleurs.
back_library = Bibliothèque
contents = Table des matières
done = Terminé
reader_gestures = Touchez les bords ou balayez pour tourner les pages. Touchez le centre pour les commandes.
view_menu = Présentation
manage = Gérer
search_library = Rechercher dans la bibliothèque
recent = Ajouts récents
title_sort = Titre
author_sort = Auteur
language = Langue

database_unavailable = Base de données indisponible. Redémarrez l’application ou vérifiez l’espace disponible.

forward = Suivant
browse = Parcourir
manage_catalogs = Gérer les catalogues
catalog_name = Nom du catalogue
add_catalog = Ajouter un catalogue OPDS
catalog_exists = Ce catalogue figure déjà dans votre liste.
remove_catalog = Supprimer le catalogue
catalog_manager_touch_hint = Maintenez le doigt sur un catalogue pour le déplacer. Touchez-le pour le modifier ou le supprimer.
catalog_manager_desktop_hint = Faites glisser les catalogues pour les réordonner. Double-cliquez sur l’un d’eux pour le modifier.
choose_publication = Sélectionnez un livre pour afficher ses détails.

genre = Genre
publisher = Éditeur
published = Publication
series = Série
contributors = Contributeurs
identifier = Identifiant
rights = Droits
updated = Mise à jour
format = Format
pages = Pages
chapters = Chapitres
file_size = Taille du fichier
about_book = À propos du livre
book_information = Informations
open_book = Ouvrir le livre
justified = Texte justifié
hyphenation = Césure
hide_status_bar = Masquer la barre d’état
left_tap_advances = Toucher à gauche avance
paragraph_spacing = Espacement des paragraphes
typography = Typographie
page_layout = Mise en page
settings_reading = Lecture
settings_reader_theme = Thème de lecture
settings_controls = Commandes

# App-owned text, including reader chrome and error messages.
error_status = Impossible de terminer : { $message }
operation_failed = L’opération n’a pas pu être terminée. Veuillez réessayer.
network_failed = Connexion impossible. Vérifiez votre connexion et réessayez.
http_failed = Le serveur a renvoyé le code HTTP { $status }.
download_limit = Le téléchargement dépasse la taille autorisée.
download_progress = Téléchargement… { $received } Mo
download_progress_total = Téléchargement… { $received } / { $total } Mo
file_size_mb = { $size } Mo
format_epub = EPUB
column_one = 1
column_two = 2
language_english = English
language_french = Français
catalog_standard = Standard Ebooks
catalog_gutenberg = Project Gutenberg
catalog_ebooks_gratuits = Ebooks Gratuits
position_text = Chapitre { $chapter }/{ $chapters } · { $page }/{ $pages }
appearance_glyph = Aa
epub_invalid = Ce fichier n’est pas un livre EPUB valide.
epub_resources_limit = Ce livre EPUB contient trop de ressources.
epub_unsafe_path = Ce livre EPUB contient un chemin de ressource non sécurisé.
epub_size_limit = Ce livre EPUB dépasse la taille maximale des ressources.
epub_resource_missing = Une ressource nécessaire à ce livre EPUB est manquante.
epub_package_missing = Le document de structure du livre EPUB est manquant.
epub_chapter_missing = Un chapitre de ce livre EPUB est manquant.
epub_no_chapters = Ce livre EPUB ne contient aucun chapitre lisible.
epub_encryption_invalid = Ce livre EPUB contient des informations de chiffrement invalides.
epub_font_invalid = Une police intégrée à ce livre EPUB est invalide ou manquante.
epub_drm = Les livres EPUB protégés par DRM ne sont pas pris en charge.
catalog_http_only = Les liens du catalogue doivent utiliser HTTP ou HTTPS.
catalog_invalid = Ce document n’est pas un catalogue OPDS valide.
search_invalid = Le format de recherche de ce catalogue n’est pas pris en charge.
search_missing = Ce catalogue ne fournit pas de modèle de recherche.
legacy_import_failed = Certains livres de la version précédente n’ont pas pu être importés.
sample_unavailable = Le livre d’exemple fourni n’a pas pu être ouvert.

library_recents = Lectures récentes
library_titles = Titres
library_authors = Auteurs
library_genres = Genres
library_recents_summary = { $n ->
    [0] Aucun livre ouvert pour l’instant
    [one] { $n } livre ouvert récemment
   *[other] { $n } livres ouverts récemment
}
library_titles_summary = { $n ->
    [0] Aucun livre pour l’instant
    [one] { $n } livre
   *[other] { $n } livres
}
library_authors_summary = { $n ->
    [0] Aucun auteur pour l’instant
    [one] { $n } auteur
   *[other] { $n } auteurs
}
library_genres_summary = { $n ->
    [0] Aucun genre pour l’instant
    [one] { $n } genre
   *[other] { $n } genres
}
unknown_author = Auteur inconnu
unknown_genre = Non classé
library_no_results = Aucun livre pour le moment.
library_recents_hint = Vos 50 derniers livres ouverts apparaissent ici.
library_select_book = Sélectionnez un livre pour afficher ses détails.
remove_book_question = Supprimer ce livre de votre bibliothèque ?

edit_catalog = Modifier le catalogue
validate_catalog = Valider
catalog_validation = Informations sur le catalogue
catalog_validating = Vérification du catalogue…
catalog_validated = Catalogue vérifié
catalog_counts = { $books } livres · { $links } liens de navigation
catalog_title = Titre publié
catalog_updated = Mise à jour
catalog_description = Description
catalog_title_hint = Choisissez le nom affiché dans votre liste de catalogues.

file_menu = Fichier
open_book_file = Ouvrir un livre…
increase_font = Agrandir le texte
decrease_font = Réduire le texte
book_import_failed = Impossible d’importer le livre.

bookmark_add = Ajouter un signet à cette page
bookmark_remove = Supprimer le signet
bookmarks = Signets
bookmarks_empty = Aucun signet. Utilisez le bouton de signet pour enregistrer une page.
chapter_position = Position dans le chapitre
return_position = Revenir à la position précédente
contents_search = Rechercher un chapitre
contents_empty = Aucun chapitre correspondant.
chapter_percent = { $percent } du chapitre

# Catalog discovery and recovery.
catalog_heading = Découvrez votre prochain livre
catalog_hint = Parcourez des catalogues indépendants et ajoutez une nouvelle histoire à votre bibliothèque.
catalog_empty = Ajoutez un catalogue pour commencer à explorer.
catalog_no_results = Aucun résultat. Essayez une autre recherche ou revenez en arrière pour parcourir le catalogue.
retry = Réessayer
refresh = Actualiser

# Catalog shelves, facets and book availability.
see_all = Tout afficher
catalog_total = { $count } livres
search_results = Résultats pour « { $query } »
sort_filter = Trier et filtrer
facet_count = { $facet } ({ $count })
facet_choice = { $group } : { $facet }
more_by = Plus de { $author }
epub_checking = Recherche d’une édition EPUB…
unavailable_account = Ce catalogue prête ou vend ce livre via un compte. Stanza Redux télécharge uniquement des livres EPUB ouverts.
unavailable_pdf = Ce catalogue propose ce livre uniquement au format PDF.
unavailable_audio = Ce catalogue propose ce livre uniquement en livre audio.
unavailable_other = Ce catalogue ne propose pas d’édition EPUB de ce livre.
view_on_web = Voir sur le site web
