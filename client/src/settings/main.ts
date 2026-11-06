import { createApp } from 'vue'
import Settings from './Settings.vue'
import { registerPwa } from '../pwa'

registerPwa()
createApp(Settings).mount('#settings')
