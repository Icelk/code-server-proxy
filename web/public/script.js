const userName = document.getElementById("name")
const code = document.getElementById("code")
const userFiles = document.getElementById("files")
const port = document.getElementById("port")
const server = document.getElementById("server")

function tryEnable(input, ...buttons) {
    let disabled = input.value.length == 0
    buttons.forEach((button) => {
        button.disabled = disabled
    })
}
function lc(string) {
    return string.toLowerCase()
}
function to(url) {
    location.href = location.href.replace(/\/.*/g, url)
}
