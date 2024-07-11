import axios from "axios";

export class HeaderService {
  async getHeaders() {
    try {
      const response = await axios.get("/assets/data/headers.json");
      return response.data.data;
    } catch (error) {
      console.error("Error fetching headers:", error);
      throw error;
    }
  }
}
