(function () {
    function mountLanding() {
        // TODO not sure if keeping this
    }

    window.pageMounts = window.pageMounts || {};
    window.pageMounts.landing = mountLanding;

    if (document.body && document.body.dataset.page === "landing") {
        // mountLanding();
    } else if (document.readyState === "loading") {
        document.addEventListener("DOMContentLoaded", () => {
            if (document.body.dataset.page === "landing") {
                // mountLanding();
            }
        });
    }
})();
