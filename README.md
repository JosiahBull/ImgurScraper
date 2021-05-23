# ImgurScraper
This project's goal is to filter political posts from my Imgur browsing experience.

This project consists of two components, a server component and chrome extension.

The server component is written in rust, and automatically downloads images sent to it by the chrome extension. It then checks the title, and runs OCR on the post. If it finds images which contain political information, it communicates back to the extension to blur (or skip!) that post entirely.

The chrome extension is written in JS, and simply sends the current page, along with upcoming pages to the rust webserver.
