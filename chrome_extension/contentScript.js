// 'use strict';

//Global Config
const SERVER_IP = '';
const CACHE_NUM = 5;
const DEBUG = false;
const MAX_NUM_CONNECTION_ATTEMPTS = 3;

//Global Vars
let cache = {};
let post_observer;
let image_range_observer;

//Collect elements from dom.
let next_nav_button = document.getElementsByClassName('navNext')[0];
let edit_monitor = document.getElementsByClassName('post-title')[0]; //When this element changes we have loaded a new post.
let section_monitor = document.getElementsByClassName('post-images')[0]; //When this element changes, the user has scrolled to a new set of images in a dump.
let toast = document.createElement("div");

//Functions
//Collects the current id of the page.
let get_id = () => document.getElementsByClassName("selected base list")[0].getAttribute("href").split("/").pop(); //This can be undefined for some reason?

//This function extends the build-in JS promises with some extra methods.
function create_custom_promise(promise) {
    if (promise.isResolved) return promise;
    if (promise.modded) return promise;

    let is_pending = true;
    let is_rejected = false;
    let is_fulfilled = false;

    let promise_observer = promise.then(res => {
        is_fulfilled = true;
        is_pending = false;
        return res;
    }).catch(err => {
        is_rejected = true;
        is_pending = false;
        throw err;
    });

    promise_observer.modded = true;
    promise_observer.is_fulfilled = () => is_fulfilled;
    promise_observer.is_rejected = () => is_rejected;
    promise_observer.is_pending = () => is_pending;
    return promise_observer;
}

//Skips to the next post when called.
function trigger_next() {
    next_nav_button.click();
};

//Shows a toast with a message at the bottom of the screen. Supports error and default.
function show_toast(message, type) {
    let x = document.getElementById("toast");
    console.log("Showing Toast");
    switch (type) {
        case 'error': {
            x.style.backgroundColor = "#850900";
            break;
        }
        default: {
            x.style.backgroundColor = "#333";
        }
    }

    x.innerText = message;
    x.className = "show";
    setTimeout(function(){ x.className = x.className.replace("show", ""); }, 3000);
}

//Makes a request to the server for 
async function request_post(post_id) {
    return await axios({
        method: 'post',
        url: `${SERVER_IP}/check_post_priority`,
        data: JSON.stringify({
            id: post_id,
            images: [],
            post_url: `https://imgur.com/gallery/${post_id}`,
            datetime: Date.now().toString()
        }),
        headers: {
            'Content-Type': 'application/json'
        }
    }).then(response => {
        if (response.status !== 200) throw new Error('Error! Server returned non-200 status.');
        return response.data;
    })
}

async function get_ahead() {
    let gallery = document.getElementsByClassName("base list");
    let current_index = 0;

    for (let item of gallery) {
        if (item.classList.contains("selected")) break;
        current_index++;
    }

    for(let i = 1; i <= CACHE_NUM; i++) {
        let id = gallery[current_index + i].getAttribute("href").split("/").pop();
        if (!cache.hasOwnProperty(id)) cache[id] = create_custom_promise(request_post(id));
    }
}

async function scan_page_core(retry_num) {
    try {
        check_images(); //Hide any relevant images.
        //Get current element from upcoming Posts
        let id = get_id();
        let post_data = (cache.hasOwnProperty(id)) ? cache[id] : create_custom_promise(request_post(id));
        if (!cache.hasOwnProperty(id)) cache[id] = post_data;

        await post_data.then(result => {
            // console.log(result);
            if (result.unrecoverable) {
                trigger_next();
                show_toast("Skipped Post!");
            }
        }).catch(err => {
            if (DEBUG) show_toast("Error contacting server", 'error');
            if (DEBUG) console.error(`An error occured while attempting to collect data from the server: ${err.stack}`);;
        });
        check_images();
        get_ahead(); //Hide any relevant images.
        return post_data;
    } catch (err) {
        if (DEBUG) console.error(`Political Post Blocker: Failed to acquire post data. Attempt ${retry_num} of ${MAX_NUM_CONNECTION_ATTEMPTS}. \n${err.stack}`);
        if (retry_num < MAX_NUM_CONNECTION_ATTEMPTS) setTimeout(() => scan_page_core(retry_num + 1), 500);
    }
}

function blur_image(image) {
    image.style.filter = 'blur(5px)';
}

function check_images() {
    try {
        let id = get_id();
        if (cache.hasOwnProperty(id)) {
            if (cache[id].is_fulfilled) { //Check if cached promise has resolved
                cache[id].then(val => {
                    let images = document.getElementsByClassName('post-image-container');
                    for (let image of images) {
                        let id = image.getAttribute('id');
                        let relevant_image = undefined;
                        let i = 0;
                        while (relevant_image === undefined) {
                            if (val.images[i].id = id) relevant_image = val.images[i];
                            i++;
                        }
                        if (relevant_image.unrecoverable) blur_image(image);
                    }
                })
            }
        }
    } catch (err) {
        if (DEBUG) console.error(`Failed to censor images: ${err.stack}`);
    }
};

let scan_page = async () => setTimeout(() => scan_page_core(1), 500); //Delay the call by 500ms to let the DOM load fully.



//Inital call to scan page, to start program.
window.onload = () => {
    //Initalise the toast div.
    toast.setAttribute("id", "toast");
    document.body.appendChild(toast);

    //This observer controls when the user clicks to a new post.
    post_observer = new MutationObserver(scan_page);
    post_observer.observe(edit_monitor, { attributes: true, childList: true, subtree: true });

    //This observer is responsible for the subset of loaded images changing.
    image_range_observer = new MutationObserver(check_images);
    post_observer.observe(section_monitor, {attributes: true, childList: true, subtree: true});

    scan_page();
};