import axios from "axios";

export class HeaderService {
  getHeaders(_this) {
    return axios
      .get("/assets/data/headers.json")
      .then((res) => res.data.data)
      .then((data) => {
        _this.setState({ headersData: data });
        return data;
      });
  }
}
