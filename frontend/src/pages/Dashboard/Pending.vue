<script setup lang="ts">
import {ref, onMounted, watch} from "vue";
import LoadingCircle from "../../components/LoadingCircle.vue";

interface PendingItem {
  id: number;
  user_id: number;
  username: string;
  level_id: number;
  accepted: boolean;
  replacement: boolean;
  upload_time: string;
}

const loading = ref(true);
const error = ref<string | null>(null);
const pendingItems = ref<PendingItem[]>([]);
const selectedItem = ref<PendingItem | null>(null);

const rejectReasonField = ref<HTMLInputElement | null>(null);
const rejectReason = ref<string>("");

const fullscreenLoading = ref(false);

watch(selectedItem, () => {
  rejectReason.value = "";
});

onMounted(async () => {
  try {
    const response = await fetch('/pending');
    const data = await response.json();

    // Check if the response is ok
    if (!response.ok) {
      throw new Error(data.message || 'Failed to fetch pending items');
    }

    pendingItems.value = data;
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'An unknown error occurred';
  } finally {
    loading.value = false;
  }
});

async function thumbnailAction(id: number, accept: boolean) {
  if (!selectedItem.value || fullscreenLoading.value) return;
  if (!accept) {
    // make sure the reject reason is provided
    if (rejectReason.value.trim() === "") {
      rejectReasonField.value!.focus();
      return;
    }
  }

  fullscreenLoading.value = true;

  try {
    const response = await fetch(`/pending/${id}`, {
      method: 'POST',
      headers: {'Content-Type': 'application/json'},
      body: JSON.stringify({
        accepted: accept,
        reason: accept ? null : rejectReason.value.trim()
      })
    });

    if (!response.ok) {
      const data = await response.json();
      throw new Error(data.message || 'Failed to process thumbnail action');
    }

    // remove the item from the list
    pendingItems.value = pendingItems.value.filter(item => item.id !== id);
    selectedItem.value = null;
  } catch (err) {
    alert("An error occurred while processing the thumbnail action: " + (err instanceof Error ? err.message : 'Unknown error'));
  } finally {
    fullscreenLoading.value = false;
  }
}
</script>

<template>
  <div v-if="loading" class="d-flex flex-middle h-100">
    <LoadingCircle/>
  </div>
  <div v-else-if="error" class="error-message">
    <img src="/error.svg" alt="Error Icon" style="width: 128px; height: auto;"/>
    <p>{{ error }}</p>
  </div>
  <div v-else>
    <div v-if="pendingItems.length === 0" class="text-center">
      <p>No pending items found.</p>
    </div>
    <div v-else-if="selectedItem" class="selected-item page-transition">
      <button @click="selectedItem = null" class="btn btn-secondary">Back to Thumbnails</button>

      <h3 class="text-center">
        Level ID: {{ selectedItem!.level_id }}
        <span v-if="selectedItem!.replacement">(Replacement)</span><br/>
        Submitted by: {{ selectedItem!.username }}
      </h3>

      <img :src="`/pending/${selectedItem!.id}/image`" alt="Selected Image" class="thumbnail-image mb-1"/>
      <div class="filler"></div>

      <div class="d-flex flex-col gap-1 w-100 sensitive-actions">
        <button @click="thumbnailAction(selectedItem!.id, true)" class="btn btn-success">
          Accept
        </button>
        <div class="d-flex">
          <input type="text" ref="rejectReasonField" v-model="rejectReason" required class="flex-3 form-control"
                 placeholder="Reason for rejection" list="rejectReasons"/>

          <datalist id="rejectReasons">
            <option value="Progress bar/percentage"/>
            <option value="Using Noclip"/>
            <option value="Low Quality"/>
            <option value="JPEGgy"/>
            <option value="Stretched"/>
            <option value="Title Card"/>
            <option value="Overlays"/>
          </datalist>

          <button @click="thumbnailAction(selectedItem!.id, false)" class="btn btn-danger flex-1">
            Reject
          </button>
        </div>
      </div>
    </div>
    <div class="image-grid page-transition" v-else>
      <div v-for="item in pendingItems" :key="item.id" class="image-item" @click="selectedItem = item">
        <img :src="`/pending/${item.id}/image`" alt="Thumbnail" class="thumbnail-image"/>
        <div class="thumbnail-info">
          By {{ item.username }}<br/>
          Level ID: {{ item.level_id }}<br/>
        </div>
      </div>
    </div>
  </div>
  <LoadingCircle backdrop v-if="fullscreenLoading"/>
</template>

<style scoped>
.error-message {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  text-align: center;
  font-size: 1.2em;
}

.image-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 2fr));
  gap: 16px;
}

.image-item {
  cursor: pointer;
  border-radius: 8px;
  overflow: hidden;
  transition: transform 0.2s;
  position: relative;
}

.image-item:hover {
  transform: scale(1.05);
}

.image-item:hover > .thumbnail-info {
  opacity: 1;
}

.thumbnail-image {
  width: 100%;
  height: auto;
  display: block;
}

.thumbnail-info {
  padding: 8px;
  position: absolute;
  bottom: 0;
  color: #fff;
  background: linear-gradient(to top, rgba(0, 0, 0, 0.7), transparent);
  width: 100%;
  opacity: 0;
  transition: opacity 0.3s ease;
}

.thumbnail-image {
  width: 100%;
  height: auto;
  border-radius: 8px;
}

.selected-item {
  display: flex;
  flex-direction: column;
  align-items: center;
  margin-top: 20px;
  height: 100%;
}

.selected-item > img {
  max-width: 800px;
  max-height: 80vh;
  object-fit: contain;
}

.sensitive-actions {
  max-width: 800px;
}

.sensitive-actions > div {
  display: flex;
  justify-content: space-between;
}

</style>