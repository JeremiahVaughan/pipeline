(function () {
    function mountLanding() {
        const input = document.querySelector("#search");
        if (!input || input.dataset.bound === "true") {
            return;
        }
        input.dataset.bound = "true";
        input.addEventListener("input", (e) => {
            console.log("Value changed to:", e.target.value);
            WS.send("search_services:" + e.target.value)
        });
    }

    window.pageMounts = window.pageMounts || {};
    window.pageMounts.landing = mountLanding;

    if (document.body && document.body.dataset.page === "landing") {
        mountLanding();
    } else if (document.readyState === "loading") {
        document.addEventListener("DOMContentLoaded", () => {
            if (document.body.dataset.page === "landing") {
                mountLanding();
            }
        });
    }
})();
