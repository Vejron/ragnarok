import { createApp } from 'vue'
import App from './App.vue'
import { registerPwa } from './pwa'

registerPwa()
createApp(App).mount('#app')
