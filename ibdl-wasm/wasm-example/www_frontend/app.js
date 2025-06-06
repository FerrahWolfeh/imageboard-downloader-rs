import init, { fetch_links, fetch_links_proxy } from '../wpack/ibdl_wasm.js';

async function main() {
    await init();

    const siteSelect = document.getElementById('siteSelect');
    const tagsInput = document.getElementById('tagsInput');
    const limitInput = document.getElementById('limitInput');
    const searchButton = document.getElementById('searchButton');
    const proxyToggle = document.getElementById('proxyToggle');
    const resultsDiv = document.getElementById('results');
    const statusDiv = document.getElementById('status');

    searchButton.addEventListener('click', async () => {
        const site = siteSelect.value;
        const tags = tagsInput.value.trim();
        const limit = parseInt(limitInput.value, 10);
        const useProxy = proxyToggle.checked;

        if (!tags) {
            statusDiv.textContent = 'Please enter tags to search.';
            return;
        }

        resultsDiv.innerHTML = '';
        statusDiv.textContent = `Searching on ${site} for "${tags}"${useProxy ? " (via proxy)" : ""}...`;
        searchButton.disabled = true;

        try {
            let posts;
            if (useProxy) {
                console.log("Using proxy function: fetch_links_proxy");
                posts = await fetch_links_proxy(site, tags, limit);
            } else {
                console.log("Using direct function: fetch_links");
                posts = await fetch_links(site, tags, limit);
            }


            if (posts?.length > 0) {
                statusDiv.textContent = `Found ${posts.length} posts.`;
                posts.forEach(post => {
                    const item = document.createElement('div');
                    item.classList.add('post-item');
                    // Use textContent for security, and build HTML safely
                    const tagsHtml = post.tags.map(tag => `<span>${tag}</span>`).join(', ');
                    item.innerHTML = `
                        <h3>Post #${post.id} <small>(${post.rating})</small></h3>
                        <p><a href="${post.post_url}" target="_blank">View Post on ${post.site}</a></p>
                        <p><a href="${post.direct_url}" target="_blank">Direct Link</a></p>
                        <p class="tags">Tags: ${tagsHtml}</p>
                    `;
                    resultsDiv.appendChild(item);
                });
            } else {
                statusDiv.textContent = 'No posts found.';
            }
        } catch (error) {
            console.error("Error from WASM:", error);
            statusDiv.textContent = `Error: ${error}`;
        } finally {
            searchButton.disabled = false;
        }
    });
}

main().catch(console.error);