// Enhanced never uploads to upstream. Only an explicit same-origin loopback
// fixture can enable the old report UI; a future fork service belongs here.
export function reportsEndpoint(href) {
  try {
    const page = new URL(href);
    if (page.protocol !== 'http:' || !(/^127(?:\.\d{1,3}){3}$/.test(page.hostname) || page.hostname === '[::1]')) return null;
    const explicit = page.searchParams.get('reports');
    if (!explicit || /[\\\s?#%]/.test(explicit)) return null;
    const endpoint = new URL(explicit, page);
    if (endpoint.origin !== page.origin || endpoint.username || endpoint.password || endpoint.search || endpoint.hash) return null;
    return endpoint.href.replace(/\/+$/, '');
  } catch {
    return null;
  }
}
