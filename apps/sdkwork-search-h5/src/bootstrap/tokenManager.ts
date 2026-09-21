/**
 * SCAFFOLD PLACEHOLDER — not wired into `src/main.tsx` or `src/App.tsx`.
 *
 * This app root is still at the scaffolding stage: every module in
 * `src/bootstrap/` returns an empty stub and nothing consumes them, so this
 * factory carries no credential authority yet. It is exempted in
 * `sdkwork-specs/tools/check-token-manager-bootstrap-fallback.mjs` for exactly
 * that reason.
 *
 * When the real runtime lands, this factory MUST become a proper
 * `AuthTokenManager` that seeds its Access-Token from the private bootstrap
 * artifact (`APP_SDK_INTEGRATION_SPEC.md` section 4), most likely by binding a
 * session store the way the other application roots do. Do not ship this stub.
 */
export function createTokenManager() {
  return {};
}
