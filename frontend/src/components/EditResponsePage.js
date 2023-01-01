import React, { Component } from 'react';
import { Button } from 'primereact/button';
import { InputText } from 'primereact/inputtext';
import { AutoComplete } from 'primereact/autocomplete';
import { HeaderService } from '../service/HeaderService';
import AceEditor from 'react-ace';
import 'ace-builds/src-min-noconflict/ext-language_tools';
import 'ace-builds/src-noconflict/theme-github';
import 'ace-builds/src-noconflict/mode-html';
import { FileUpload } from 'primereact/fileupload';
import { HeaderInput } from './HeaderInput';
import { Utils } from "../Utils";
import { toast } from "react-toastify";

export class EditResponsePage extends Component {
	constructor(props) {
		super(props);
		this.state = {
			filteredHeaders: null,
			headers: (this.props.headers ? this.props.headers : []),
			content: (this.props.content ? this.props.content : ""),
			fetched: (this.props.fetched ? this.props.fetched : false),
			statusCode: (this.props.statusCode ? this.props.statusCode : 200)
		};

		if (!this.state.fetched) {
			Utils.getFile().then(res => {
				this.setState({ headers: res['headers'] });
				try {
					this.setState({ content: atob(res['raw']) });
				} catch { }
				this.setState({ statusCode: res['status_code'] });
				this.setState({ fetched: true });
			});
		}

		this.HeaderService = new HeaderService();
		this.HeaderService.getHeaders(this);
		this.add = this.add.bind(this);
		this.handleHeaderInputChange = this.handleHeaderInputChange.bind(this);
		this.contentChange = this.contentChange.bind(this);
		this.saveChanges = this.saveChanges.bind(this);
	}

	updateDimensions = () => {
		this.setState(this.state);
	};
	componentDidMount() {
		window.addEventListener('resize', this.updateDimensions);
	}
	componentWillUnmount() {
		window.removeEventListener('resize', this.updateDimensions);
	}

	componentDidUpdate(prevProps) {
		// will be true
	}

	saveChanges() {
		let obj = {};
		obj['headers'] = this.state.headers.filter(function (value) {
			return value.header.length > 0;
		});
		obj['status_code'] = this.state.statusCode;
		obj['raw'] = btoa(this.state.content);
		Utils.updateFile(obj).then(res => {
			if (res.error) {
				this.props.toast.error(res.error, {
					position: "bottom-center",
					autoClose: 4000,
					hideProgressBar: false,
					closeOnClick: true,
					pauseOnHover: true,
					draggable: true
				});
			} else {
				this.props.toast.success(res.msg, {
					position: "bottom-center",
					autoClose: 4000,
					hideProgressBar: false,
					closeOnClick: true,
					pauseOnHover: true,
					draggable: true
				});
				Utils.getFile().then(res => {
					this.setState({ headers: res['headers'] });
					try {
						this.setState({ content: atob(res['raw']) });
					} catch { }
					this.setState({ statusCode: res['status_code'] });
					this.setState({ fetched: true });
				});
			}
		});
	}

	add(header, value) {
		if (typeof header !== 'string') header = '';
		if (typeof value !== 'string') value = '';
		const { headers } = this.state;
		headers.push({ header: header, value: value });
		this.setState({ headers: headers });
	}

	isDesktop() {
		return window.innerWidth > 1180;
	}

	handleHeaderInputChange(index, header, value, toDelete) {
		const headers = this.state.headers;
		if (toDelete === false) {
			headers[index] = { header: header, value: value };
		} else {
			headers.splice(index, 1);
		}
		this.setState({ headers: headers });
		this.setState(this.state);
	}

	contentChange(event) {
		this.setState({ content: event });
	}

	render() {
		let headers = [];
		headers = this.state.headers.map((element, index) => {
			return (
				<HeaderInput
					key={index}
					index={index}
					header={element['header']}
					value={element['value']}
					handleHeaderInputChange={this.handleHeaderInputChange}
					headersData={this.state.headersData}
				/>
			);
		});
		return (
			<div
				className="card card-w-title"
				style={{ border: '1px solid #cccccc', borderTop: '0px', borderRadius: '0px 0px 5px 5px' }}
			>
				<div className="grid">
					<div className="col-12">
						<div className="grid">
							<div className="col-6">
								<h1>Edit Response</h1>
							</div>
							<div className="col-6">
								<Button
									label="Save changes"
									icon="pi pi-save"
									className="p-button-text p-button-success"
									style={{float: 'right' }}
									onClick={this.saveChanges}
								/>
							</div>
						</div>
						<AceEditor
							placeholder=""
							mode="html"
							theme="github"
							onLoad={this.onLoad}
							onChange={this.contentChange}
							fontSize={14}
							showPrintMargin={true}
							showGutter={true}
							highlightActiveLine={true}
							width={'100%'}
							height={'400px'}
							style={{ border: '1px solid black' }}
							value={this.state.content}
							setOptions={{
								enableBasicAutocompletion: true,
								enableLiveAutocompletion: false,
								enableSnippets: false,
								showLineNumbers: true,
								tabSize: 2
							}}
						/>
						<h1>Status Code</h1>
						<InputText value={this.state.statusCode} onChange={(e) => {
							if (e.target.value.length < 10 && /^[0-9]*$/.test(e.target.value)) {
								this.setState({ statusCode: e.target.value })
							}
						}
						} />
					</div>
					<div className="col-12">
						<div className="grid">
							<div className="col-6">
								<h1>
									Response HTTP Headers{' '}
								</h1>
							</div>
							<div className="col-6">
									<Button
										label="Add header"
										onClick={this.add}
										icon="pi pi-plus"
										className='p-button-text'
										style={{ float: 'right', top: '-3px' }}
									/>
							</div>
						</div>
						<div>
							{headers}
						</div>
					</div>
					<div style={{ width: '100%' }}>

					</div>
				</div>
			</div>
		);
	}
}
