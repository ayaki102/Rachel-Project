All in one web "pentesting tool". give it scope and it will do the rest

1. enumerates webapp with given endpoints and returns info about any "input fields/buttons"
  it comes across


what i need from user
  -> filled file i generated for them

file specs:
  target -> just url

what scanner needs to check for:

hidden inputs: type=hidden often store tokens flag them and check values.
default value leakage: check value attribute for high entropy or token-like patterns (^[A-Za-z0-9_\-]{20,}$).
autocomplete misuse: e.g., autocomplete="on" on password or payment fields; autocomplete="cc-number" presence.
password fields mis-typed: type="text" used for password-like names.
form action: insecure http:// action when the page is https://.
client-side validation bypass: pattern or maxlength presence but you must test server-side by sending longer or invalid payloads.
XSS reflection: inject payload into field, submit, then search response for reflection unsanitized.
CSRF detection: look for tokens in forms (hidden inputs with names like csrf, __RequestVerificationToken) and/or absence of token on state-changing forms.
file inputs: check accept restrictions and multiple attribute.
event handlers: detect on* attributes (e.g., onchange, oninput) can be exploited by DOM-based XSS.
data- misuse*: sensitive data in data-* attributes.
