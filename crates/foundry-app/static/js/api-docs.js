// api-openapi-document slice 03 — the API page's initializer (DDD-18, DDD-20).
//
// Authored in tree, served `no-cache` from /static/js/. Loaded `defer` after the
// vendored swagger-ui bundle, so `SwaggerUIBundle` exists when this runs. The
// page's policy grants `script-src 'self'` and no `'unsafe-eval'`
// (ADR-API-OPENAPI-004), so this file holds every line of page script: no
// inline script, no inline handler, no eval.
//
// Every option that touches D10 is set explicitly, so a changed default in a
// future swagger-ui release cannot loosen it:
//   * url is fixed; there is no urls, spec or configUrl, and
//     queryConfigEnabled is false, so no link can choose the description or the
//     configuration (the 4.1.3 advisory class);
//   * validatorUrl "none" — unset means the badge posts the description to
//     validator.swagger.io;
//   * persistAuthorization false — swagger-ui's only localStorage write path,
//     so a pasted token lives in this tab's memory and a reload forgets it.
//
// The plugin below is DDD-20 (a): swagger-ui has no option for the token
// field's label, so it re-renders the bearer branch of the HttpAuth component
// with the D14 copy. It mirrors swagger-ui 5.33.3's own bearer branch
// (heading, "Authorized", description, field, errors) and changes only the
// label, the hint, and that the field asks the browser to remember nothing.
// Every other scheme falls through to the original component. The browser lane
// pins this coupling: "The token field says where the token goes".
(function () {
  "use strict";

  var DOCUMENT_URL = "/api/v1/openapi.json";
  var TOKEN_LABEL = "Machine token";
  var TOKEN_HINT =
    "Kept in this tab only and sent only to this Foundry. Reloading forgets it.";

  function machineTokenField(Original, system) {
    var h = system.React.createElement;

    return function MachineTokenAuth(props) {
      var schema = props.schema;
      if ((schema.get("scheme") || "").toLowerCase() !== "bearer") {
        return h(Original, props);
      }
      var name = props.name;
      var getComponent = props.getComponent;
      var Input = getComponent("Input");
      var Row = getComponent("Row");
      var Col = getComponent("Col");
      var AuthError = getComponent("authError");
      var Markdown = getComponent("Markdown", true);
      var JumpToPath = getComponent("JumpToPath", true);
      var authorized = props.authorized && props.authorized.getIn([name, "value"]);
      var errors = props.errSelectors.allErrors().filter(function (error) {
        return error.get("authId") === name;
      });
      var onChange = function (event) {
        props.onChange({ name: name, schema: schema, value: event.target.value });
      };

      return h(
        "div",
        null,
        h(
          "h4",
          null,
          h("code", null, name),
          "  (http, Bearer)",
          h(JumpToPath, { path: props.authSelectors.selectAuthPath(name) })
        ),
        authorized && h("h6", null, "Authorized"),
        h(Row, null, h(Markdown, { source: schema.get("description") })),
        h(
          Row,
          null,
          h("label", { htmlFor: "auth-bearer-value" }, TOKEN_LABEL),
          authorized
            ? h("code", null, " ****** ")
            : h(
                Col,
                null,
                h(Input, {
                  id: "auth-bearer-value",
                  type: "text",
                  autoComplete: "off",
                  spellCheck: false,
                  "aria-label": TOKEN_LABEL,
                  "aria-describedby": "auth-bearer-hint",
                  onChange: onChange,
                  autoFocus: true,
                })
              ),
          h("p", { id: "auth-bearer-hint" }, TOKEN_HINT)
        ),
        errors.valueSeq().map(function (error, key) {
          return h(AuthError, { error: error, key: key });
        })
      );
    };
  }

  function MachineTokenPlugin() {
    return { wrapComponents: { HttpAuth: machineTokenField } };
  }

  window.SwaggerUIBundle({
    url: DOCUMENT_URL,
    dom_id: "#swagger-ui",
    layout: "BaseLayout",
    queryConfigEnabled: false,
    validatorUrl: "none",
    persistAuthorization: false,
    withCredentials: false,
    deepLinking: false,
    tryItOutEnabled: false,
    plugins: [MachineTokenPlugin],
  });
})();
