export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    if (url.pathname === "/health") {
      return Response.json({ status: "ok", tenant: env.TENANT_NAME ?? "unknown" });
    }
    return new Response("not found", { status: 404 });
  },
};
