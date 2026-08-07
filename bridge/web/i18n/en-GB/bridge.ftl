# systemprompt-internal-bridge en-GB message catalog.
#
# web/js/i18n.js loads en-US first and merges the negotiated locale over it, so
# this file carries only the keys whose British spelling differs. Everything
# else inherits from core's en-US catalog and must NOT be duplicated here — a
# copied key would silently pin itself to whatever en-US said the day it was
# copied.
#
# Without this file the GUI requests /assets/i18n/en-GB/bridge.ftl on every
# en-GB host and logs a 404 warning before falling back.

profile-section-models = Favourite models
