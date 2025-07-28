<script setup lang="ts">
const props = defineProps<{
  url: string;
  isDark?: boolean;
}>();
</script>

<template>
  <a :href="props.url" target="_blank" rel="noopener noreferrer" :class="{ 'dark-button': props.isDark }">
    <slot></slot>
  </a>
</template>

<style scoped>
a {
  font-family: "Afacad", "Alata", system-ui, Avenir, Helvetica, Arial, sans-serif;
  background-color: #000000;
  color: white;
  border: 2px solid #ffffff;
  border-radius: 8px;
  padding: 10px 20px;
  cursor: pointer;
  font-size: 1.2em;
  transition: transform 0.2s ease;
  min-width: 220px;
  box-shadow: 0 4px 10px rgba(0, 0, 0, 0.3);
  text-decoration: none;
  display: inline-block;
}

.dark-button {
  background-color: #ffffff;
  color: #000000;
  border-color: #000000;
}

a:hover {
  transform: scale(1.1);
}

a:active {
  transform: scale(1.05);
}
</style>