//Config
const filter_word_list = ['trump', 'putin'];
const server_address = '';
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

function replace_container(element, replacement_image) {
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

async function get_post_from_server(post_information, options) {
    let { priority } = options || false;
    return await axios({
        method: 'get',
        url: `${server_address}/check_post${(priority) ? '_priority' : ''}`,
        data: post_information
    }).then(response => {
        if (response.status !== 200) throw new Error('Error! Server returned non-200 status.');
        return response.data.post;
    }).catch(err => {
        if (display_error) console.log(`An error occured attempting to get a post from the server: ${err}`);
        return {
            err: true
        }
    })
}

async function scan_page() {
    let post_id = window.location.href.split('/').pop();
    let images = document.getElementsByClassName('post-image-container');

    if (!cache.hasOwnProperty(post_id)) {
        if (display_info) console.warn('Warning: Posts not caching correctly, requesting priority.');
        warn_unloaded(true);
        let send_images = [];
        for (image_container of images) {
            let new_image = {
                id: image_container.getAttribute('id'),
                description: image_container.children[1].children[0].children[0].innerHtml,
                url: image_container.children[0].children[0].src,
            };
            send_images.push(new_image);
        }
        let result = await get_post_from_server({
            id: post_id,
            images: send_images,
            post_url: window.location.href,
            datetime: Date.now()
        }, {
            priority: true
        });
        warn_unloaded(false);
        if (result.err = true) {
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
        for (image_container of images) {
            let image_id = image_container.getAttribute('id');
            let description = image_container.children[1].children[0].children[0].innerHtml.toLowerCase().replace(/[.,\/#!$%\^&\*;:{}=\-_`~()]/g,"");
            if (post[image_id].unrecoverable || filter_word_list.filter(word => description.includes(word)).length > 0) replace_container(image_container, post[image_id].replacement);
        }
    }
}

//Event listners
document.addEventListener('keyup', async (event) => {
    let key = event.key || event.keyCode;
    if (key === '37' || key === '39') await scan_page();
});

next_nav_button.addEventListener('click', async (event) => {
    await scan_page();
});

scan_page();