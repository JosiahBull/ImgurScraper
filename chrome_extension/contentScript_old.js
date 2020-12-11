//Config
const server_address = 'http://127.0.0.1:3030';
const display_errors = true;
const display_info = true;

//Constructors

const ErrorBox = function(message) {
    return `<div>${message}</div>`;
}

const WarnBox = function(message) {
    return `<div>${message}</div>`;
}

//Global Vars
let cache = {};
let replacement_images = [];
let current_image = null;

//Collect elements from dom.
let next_nav_button = document.getElementsByClassName('navNext')[0];

//Functions
function trigger_next() {
    next_nav_button.click();
};

function hide_vote_buttons() {

};

function remove_hidden_vote_buttons() {

};

function replace_container(element) {
    element.style.background = 'green';
    for (child of element.children) {
        child.style.background = 'green';
    }
}

function warn_unloaded() {
    
}

function display_warning(message) {
    let new_element = document.createElement('div');
    new_element.innerHTML = WarnBox(message).trim();
    document.body.appendChild(new_element.firstChild); 
}

function display_error(message) {
    let new_element = document.createElement('div');
    new_element.innerHTML = ErrorBox(message).trim();
    document.body.appendChild(new_element.firstChild);
}

async function get_post_from_server(post_information) {
    console.log(JSON.stringify(post_information));
    return await axios({
        method: 'post',
        url: `${server_address}/check_post_priority`,
        data: JSON.stringify(post_information),
        headers: {
            'Content-Type': 'application/json'
        }
    }).then(response => {
        if (response.status !== 200) throw new Error('Error! Server returned non-200 status.');
        let result = response.data;
        result.err = false;
        return result;
    }).catch(err => {
        if (display_error) console.log(`An error occured attempting to get a post from the server: ${err}`);
        return {
            err: true
        }
    })
}

async function scan_ahead() {
    
}

async function scan_page() {
    let post_id = window.location.href.split('/').pop();
    console.log(`Scanning page: ${post_id}`);
    current_post = null;

    if (!cache.hasOwnProperty(post_id)) {
        if (display_info) console.warn('Warning: Posts not caching correctly, requesting priority.');
        warn_unloaded(true);
        let result = await get_post_from_server({
            id: post_id,
            images: [],
            post_url: window.location.href,
            datetime: Date.now().toString()
        });
        warn_unloaded(false);
        if (result.err === true) {
            if (display_errors) console.log('Unable to contact server to filter posts.');
            display_error('Unable to contact server to filter post.');
            return;
        } else {
            cache[post_id] = result;
        }
    }
    let post = cache[post_id];
    if (post.unrecoverable) trigger_next();
    else {
        current_post = post.images.reduce((culm, curr) => {
            culm[curr.id] = curr
            return culm;
        }, {});
        update_images()
    }
}

function update_images() {
    if (current_post === null) return;
    
    let images = document.getElementsByClassName('post-image-container');
    for (image_container of images) {
        let image_id = image_container.getAttribute('id');
        if (current_post[image_id].unrecoverable) replace_container(image_container);
    }
}

//Event listners
document.addEventListener('keyup', async (event) => {
    let key = event.key || event.keyCode;
    if (key === '37' || key === '39') setTimeout(() => scan_page().then(() => console.log('done')), 500);
});

document.addEventListener('scroll', async () => {
    update_images();
});

next_nav_button.addEventListener('click', async (event) => {
    setTimeout(() => scan_page().then(() => console.log('done')), 300);
});

scan_page();