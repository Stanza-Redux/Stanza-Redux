import json

with open('/opt/src/github/appfair/Stanza-Redux/Sources/Stanza/Resources/Localizable.xcstrings', 'r') as f:
    data = json.load(f)

for key, value in data['strings'].items():
    comment = value.get('comment', '')
    print(f"KEY: {key}")
    print(f"COMMENT: {comment}")
    en_val = key
    if 'localizations' in value and 'en' in value['localizations']:
        en_val = value['localizations']['en']['stringUnit']['value']
    print(f"EN: {en_val}")
    print("---")
