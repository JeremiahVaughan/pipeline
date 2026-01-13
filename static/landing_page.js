(function () {
    const input = document.querySelector("#search");
    input.addEventListener("input", (e) => {
        console.log("Value changed to:", e.target.value);
        WS.send("search_services:" + e.target.value)
    });
})();
