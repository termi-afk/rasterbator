
// The Rasterbator Standalone Version 1.0
// Copyright (C) 2004-2005 Matias Ärje
// 
// This program is free software; you can redistribute it and/or
// modify it under the terms of the GNU General Public License
// as published by the Free Software Foundation; either version 2
// of the License, or (at your option) any later version.
// 
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
// 
// You should have received a copy of the GNU General Public License
// along with this program; if not, write to the Free Software
// Foundation, Inc., 59 Temple Place - Suite 330, Boston, MA  02111-1307, USA.

using System;
using System.IO;
using System.Web.UI;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Windows.Forms;
using System.Threading;
using System.Collections.Specialized;

namespace Rasterbator
{
	/// <summary>
	/// Description of MainForm.
	/// </summary>
	public class MainForm : System.Windows.Forms.Form
	{
    private System.ComponentModel.IContainer components;
    private System.Windows.Forms.TextBox textBox2;
    private System.Windows.Forms.Panel Panel4_OutputSize;
    private System.Windows.Forms.Panel Panel1_Welcome;
    private System.Windows.Forms.ProgressBar progressBar1;
    private System.Windows.Forms.Button button12;
    private System.Windows.Forms.Button button13;
    private System.Windows.Forms.Button button10;
    private System.Windows.Forms.RadioButton radioButton4;
    private System.Windows.Forms.Button button1;
    private System.Windows.Forms.Button button7;
    private System.Windows.Forms.Button button6;
    private System.Windows.Forms.ComboBox comboBox1;
    private System.Windows.Forms.ComboBox comboBox2;
    private System.Windows.Forms.ComboBox comboBox3;
    private System.Windows.Forms.ColorDialog colorDialog1;
    private System.Windows.Forms.CheckBox checkBox2;
    private System.Windows.Forms.Button button8;
    private System.Windows.Forms.RadioButton radioButton2;
    private System.Windows.Forms.RadioButton radioButton3;
    private System.Windows.Forms.RadioButton radioButton1;
    private System.Windows.Forms.NumericUpDown numericUpDown1;
    private System.Windows.Forms.Panel CustomColorDisplay;
    private System.Windows.Forms.RadioButton radioButton5;
    private System.Windows.Forms.NumericUpDown numericUpDown2;
    private System.Windows.Forms.NumericUpDown numericUpDown5;
    private System.Windows.Forms.NumericUpDown numericUpDown4;
    private System.Windows.Forms.Label label2;
    private System.Windows.Forms.Label label1;
    private System.Windows.Forms.Button button15;
    private System.Windows.Forms.Label label7;
    private System.Windows.Forms.Label label6;
    private System.Windows.Forms.Label label5;
    private System.Windows.Forms.Label label4;
    private System.Windows.Forms.Panel Panel3_PaperSize;
    private System.Windows.Forms.Label label9;
    private System.Windows.Forms.Label label8;
    private System.Windows.Forms.Button button3;
    private System.Windows.Forms.Button button2;
    private System.Windows.Forms.Panel Panel2_SourceImage;
    private System.Windows.Forms.Label label12;
    private System.Windows.Forms.Label label13;
    private System.Windows.Forms.Label label10;
    private System.Windows.Forms.Label label11;
    private System.Windows.Forms.Label label16;
    private System.Windows.Forms.Label label17;
    private System.Windows.Forms.Label label14;
    private System.Windows.Forms.Label label15;
    private System.Windows.Forms.TextBox textBox1;
    private System.Windows.Forms.Label label18;
    private System.Windows.Forms.Label label21;
    private System.Windows.Forms.Label label20;
    private System.Windows.Forms.Label label23;
    private System.Windows.Forms.Label label22;
    private System.Windows.Forms.Label label25;
    private System.Windows.Forms.Label label24;
    private System.Windows.Forms.OpenFileDialog openFileDialog1;
    private System.Windows.Forms.Label label19;
    private System.Windows.Forms.Panel Panel7_OutputFilename;
    private System.Windows.Forms.Button button11;
    private System.Windows.Forms.LinkLabel linkLabel1;
    private System.Windows.Forms.Button button14;
    private System.Windows.Forms.LinkLabel linkLabel3;
    private System.Windows.Forms.LinkLabel linkLabel2;
    private System.Windows.Forms.PictureBox pictureBox1;
    private System.Windows.Forms.PictureBox pictureBox2;
    private System.Windows.Forms.Button button4;
    private System.Windows.Forms.Button button5;
    private System.Windows.Forms.Timer timer1;
    private System.Windows.Forms.SaveFileDialog saveFileDialog1;
    private System.Windows.Forms.Button button9;
    private System.Windows.Forms.CheckBox LowPriorityRasterbation;
    private System.Windows.Forms.Panel Panel5_Options;
    private System.Windows.Forms.Panel Panel9_Completed;
    private System.Windows.Forms.Label label3;
    private System.Windows.Forms.Panel Panel8_Rasterbating;
    private System.Windows.Forms.CheckBox checkBox1;
		public MainForm()
		{
			//
			// The InitializeComponent() call is required for Windows Forms designer support.
			//
			InitializeComponent();
			
			//
			// TODO: Add constructor code after the InitializeComponent() call.
			//
		}
		
		[STAThread]
		public static void Main(string[] args)
		{
			Application.Run(new MainForm());
		}
		
		#region Windows Forms Designer generated code
		/// <summary>
		/// This method is required for Windows Forms designer support.
		/// Do not change the method contents inside the source code editor. The Forms designer might
		/// not be able to load this method if it was changed manually.
		/// </summary>
		private void InitializeComponent() {
      this.components = new System.ComponentModel.Container();
      System.Resources.ResourceManager resources = new System.Resources.ResourceManager(typeof(MainForm));
      this.checkBox1 = new System.Windows.Forms.CheckBox();
      this.Panel8_Rasterbating = new System.Windows.Forms.Panel();
      this.label3 = new System.Windows.Forms.Label();
      this.Panel9_Completed = new System.Windows.Forms.Panel();
      this.Panel5_Options = new System.Windows.Forms.Panel();
      this.LowPriorityRasterbation = new System.Windows.Forms.CheckBox();
      this.button9 = new System.Windows.Forms.Button();
      this.saveFileDialog1 = new System.Windows.Forms.SaveFileDialog();
      this.timer1 = new System.Windows.Forms.Timer(this.components);
      this.button5 = new System.Windows.Forms.Button();
      this.button4 = new System.Windows.Forms.Button();
      this.pictureBox2 = new System.Windows.Forms.PictureBox();
      this.pictureBox1 = new System.Windows.Forms.PictureBox();
      this.linkLabel2 = new System.Windows.Forms.LinkLabel();
      this.linkLabel3 = new System.Windows.Forms.LinkLabel();
      this.button14 = new System.Windows.Forms.Button();
      this.linkLabel1 = new System.Windows.Forms.LinkLabel();
      this.button11 = new System.Windows.Forms.Button();
      this.Panel7_OutputFilename = new System.Windows.Forms.Panel();
      this.label19 = new System.Windows.Forms.Label();
      this.openFileDialog1 = new System.Windows.Forms.OpenFileDialog();
      this.label24 = new System.Windows.Forms.Label();
      this.label25 = new System.Windows.Forms.Label();
      this.label22 = new System.Windows.Forms.Label();
      this.label23 = new System.Windows.Forms.Label();
      this.label20 = new System.Windows.Forms.Label();
      this.label21 = new System.Windows.Forms.Label();
      this.label18 = new System.Windows.Forms.Label();
      this.textBox1 = new System.Windows.Forms.TextBox();
      this.label15 = new System.Windows.Forms.Label();
      this.label14 = new System.Windows.Forms.Label();
      this.label17 = new System.Windows.Forms.Label();
      this.label16 = new System.Windows.Forms.Label();
      this.label11 = new System.Windows.Forms.Label();
      this.label10 = new System.Windows.Forms.Label();
      this.label13 = new System.Windows.Forms.Label();
      this.label12 = new System.Windows.Forms.Label();
      this.Panel2_SourceImage = new System.Windows.Forms.Panel();
      this.button2 = new System.Windows.Forms.Button();
      this.button3 = new System.Windows.Forms.Button();
      this.label8 = new System.Windows.Forms.Label();
      this.label9 = new System.Windows.Forms.Label();
      this.Panel3_PaperSize = new System.Windows.Forms.Panel();
      this.label4 = new System.Windows.Forms.Label();
      this.label5 = new System.Windows.Forms.Label();
      this.label6 = new System.Windows.Forms.Label();
      this.label7 = new System.Windows.Forms.Label();
      this.button15 = new System.Windows.Forms.Button();
      this.label1 = new System.Windows.Forms.Label();
      this.label2 = new System.Windows.Forms.Label();
      this.numericUpDown4 = new System.Windows.Forms.NumericUpDown();
      this.numericUpDown5 = new System.Windows.Forms.NumericUpDown();
      this.numericUpDown2 = new System.Windows.Forms.NumericUpDown();
      this.radioButton5 = new System.Windows.Forms.RadioButton();
      this.CustomColorDisplay = new System.Windows.Forms.Panel();
      this.numericUpDown1 = new System.Windows.Forms.NumericUpDown();
      this.radioButton1 = new System.Windows.Forms.RadioButton();
      this.radioButton3 = new System.Windows.Forms.RadioButton();
      this.radioButton2 = new System.Windows.Forms.RadioButton();
      this.button8 = new System.Windows.Forms.Button();
      this.checkBox2 = new System.Windows.Forms.CheckBox();
      this.colorDialog1 = new System.Windows.Forms.ColorDialog();
      this.comboBox3 = new System.Windows.Forms.ComboBox();
      this.comboBox2 = new System.Windows.Forms.ComboBox();
      this.comboBox1 = new System.Windows.Forms.ComboBox();
      this.button6 = new System.Windows.Forms.Button();
      this.button7 = new System.Windows.Forms.Button();
      this.button1 = new System.Windows.Forms.Button();
      this.radioButton4 = new System.Windows.Forms.RadioButton();
      this.button10 = new System.Windows.Forms.Button();
      this.button13 = new System.Windows.Forms.Button();
      this.button12 = new System.Windows.Forms.Button();
      this.progressBar1 = new System.Windows.Forms.ProgressBar();
      this.Panel1_Welcome = new System.Windows.Forms.Panel();
      this.Panel4_OutputSize = new System.Windows.Forms.Panel();
      this.textBox2 = new System.Windows.Forms.TextBox();
      this.Panel8_Rasterbating.SuspendLayout();
      this.Panel9_Completed.SuspendLayout();
      this.Panel5_Options.SuspendLayout();
      this.Panel7_OutputFilename.SuspendLayout();
      this.Panel2_SourceImage.SuspendLayout();
      this.Panel3_PaperSize.SuspendLayout();
      ((System.ComponentModel.ISupportInitialize)(this.numericUpDown4)).BeginInit();
      ((System.ComponentModel.ISupportInitialize)(this.numericUpDown5)).BeginInit();
      ((System.ComponentModel.ISupportInitialize)(this.numericUpDown2)).BeginInit();
      ((System.ComponentModel.ISupportInitialize)(this.numericUpDown1)).BeginInit();
      this.Panel1_Welcome.SuspendLayout();
      this.Panel4_OutputSize.SuspendLayout();
      this.SuspendLayout();
      // 
      // checkBox1
      // 
      this.checkBox1.Checked = true;
      this.checkBox1.CheckState = System.Windows.Forms.CheckState.Checked;
      this.checkBox1.Font = new System.Drawing.Font("Tahoma", 15F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.checkBox1.Location = new System.Drawing.Point(88, 72);
      this.checkBox1.Name = "checkBox1";
      this.checkBox1.Size = new System.Drawing.Size(488, 24);
      this.checkBox1.TabIndex = 19;
      this.checkBox1.Text = "Draw cutout line around rasterbated area";
      // 
      // Panel8_Rasterbating
      // 
      this.Panel8_Rasterbating.Controls.Add(this.label24);
      this.Panel8_Rasterbating.Controls.Add(this.LowPriorityRasterbation);
      this.Panel8_Rasterbating.Controls.Add(this.label23);
      this.Panel8_Rasterbating.Controls.Add(this.label19);
      this.Panel8_Rasterbating.Controls.Add(this.progressBar1);
      this.Panel8_Rasterbating.Location = new System.Drawing.Point(0, 128);
      this.Panel8_Rasterbating.Name = "Panel8_Rasterbating";
      this.Panel8_Rasterbating.Size = new System.Drawing.Size(696, 392);
      this.Panel8_Rasterbating.TabIndex = 14;
      this.Panel8_Rasterbating.Visible = false;
      // 
      // label3
      // 
      this.label3.Font = new System.Drawing.Font("Tahoma", 16F, System.Drawing.FontStyle.Bold, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.label3.ForeColor = System.Drawing.SystemColors.ControlText;
      this.label3.Location = new System.Drawing.Point(80, 56);
      this.label3.Name = "label3";
      this.label3.Size = new System.Drawing.Size(368, 24);
      this.label3.TabIndex = 8;
      this.label3.Text = "Select source image (1/5)";
      // 
      // Panel9_Completed
      // 
      this.Panel9_Completed.Controls.Add(this.linkLabel3);
      this.Panel9_Completed.Controls.Add(this.label22);
      this.Panel9_Completed.Controls.Add(this.checkBox2);
      this.Panel9_Completed.Controls.Add(this.button10);
      this.Panel9_Completed.Controls.Add(this.linkLabel2);
      this.Panel9_Completed.Controls.Add(this.label20);
      this.Panel9_Completed.Controls.Add(this.label21);
      this.Panel9_Completed.Location = new System.Drawing.Point(0, 128);
      this.Panel9_Completed.Name = "Panel9_Completed";
      this.Panel9_Completed.Size = new System.Drawing.Size(696, 392);
      this.Panel9_Completed.TabIndex = 15;
      this.Panel9_Completed.Visible = false;
      // 
      // Panel5_Options
      // 
      this.Panel5_Options.Controls.Add(this.button14);
      this.Panel5_Options.Controls.Add(this.button7);
      this.Panel5_Options.Controls.Add(this.CustomColorDisplay);
      this.Panel5_Options.Controls.Add(this.radioButton5);
      this.Panel5_Options.Controls.Add(this.radioButton4);
      this.Panel5_Options.Controls.Add(this.radioButton3);
      this.Panel5_Options.Controls.Add(this.label18);
      this.Panel5_Options.Controls.Add(this.label17);
      this.Panel5_Options.Controls.Add(this.checkBox1);
      this.Panel5_Options.Controls.Add(this.label10);
      this.Panel5_Options.Controls.Add(this.label14);
      this.Panel5_Options.Controls.Add(this.numericUpDown5);
      this.Panel5_Options.Controls.Add(this.label15);
      this.Panel5_Options.Controls.Add(this.label16);
      this.Panel5_Options.Controls.Add(this.button6);
      this.Panel5_Options.Location = new System.Drawing.Point(0, 128);
      this.Panel5_Options.Name = "Panel5_Options";
      this.Panel5_Options.Size = new System.Drawing.Size(696, 392);
      this.Panel5_Options.TabIndex = 12;
      this.Panel5_Options.Visible = false;
      this.Panel5_Options.Paint += new System.Windows.Forms.PaintEventHandler(this.Panel5_OptionsPaint);
      // 
      // LowPriorityRasterbation
      // 
      this.LowPriorityRasterbation.Font = new System.Drawing.Font("Tahoma", 15F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.LowPriorityRasterbation.Location = new System.Drawing.Point(80, 320);
      this.LowPriorityRasterbation.Name = "LowPriorityRasterbation";
      this.LowPriorityRasterbation.Size = new System.Drawing.Size(232, 24);
      this.LowPriorityRasterbation.TabIndex = 22;
      this.LowPriorityRasterbation.Text = "Rasterbate on low priority";
      this.LowPriorityRasterbation.CheckedChanged += new System.EventHandler(this.LowPriorityRasterbationCheckedChanged);
      // 
      // button9
      // 
      this.button9.Font = new System.Drawing.Font("Tahoma", 15F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.button9.Location = new System.Drawing.Point(472, 336);
      this.button9.Name = "button9";
      this.button9.Size = new System.Drawing.Size(136, 32);
      this.button9.TabIndex = 7;
      this.button9.Text = "Rasterbate!";
      this.button9.Click += new System.EventHandler(this.Button9Click);
      // 
      // saveFileDialog1
      // 
      this.saveFileDialog1.Filter = "Pdf files (*.pdf)|*.pdf";
      // 
      // timer1
      // 
      this.timer1.Tick += new System.EventHandler(this.Timer1Tick);
      // 
      // button5
      // 
      this.button5.Font = new System.Drawing.Font("Tahoma", 15F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.button5.Location = new System.Drawing.Point(472, 336);
      this.button5.Name = "button5";
      this.button5.Size = new System.Drawing.Size(136, 32);
      this.button5.TabIndex = 7;
      this.button5.Text = "Continue >";
      this.button5.Click += new System.EventHandler(this.Button5Click);
      // 
      // button4
      // 
      this.button4.Font = new System.Drawing.Font("Tahoma", 15F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.button4.Location = new System.Drawing.Point(472, 336);
      this.button4.Name = "button4";
      this.button4.Size = new System.Drawing.Size(136, 32);
      this.button4.TabIndex = 7;
      this.button4.Text = "Continue >";
      this.button4.Click += new System.EventHandler(this.Button4Click);
      // 
      // pictureBox2
      // 
      this.pictureBox2.Location = new System.Drawing.Point(32, 48);
      this.pictureBox2.Name = "pictureBox2";
      this.pictureBox2.Size = new System.Drawing.Size(300, 300);
      this.pictureBox2.TabIndex = 21;
      this.pictureBox2.TabStop = false;
      // 
      // pictureBox1
      // 
      this.pictureBox1.Image = ((System.Drawing.Image)(resources.GetObject("pictureBox1.Image")));
      this.pictureBox1.Location = new System.Drawing.Point(0, 0);
      this.pictureBox1.Name = "pictureBox1";
      this.pictureBox1.Size = new System.Drawing.Size(700, 128);
      this.pictureBox1.TabIndex = 0;
      this.pictureBox1.TabStop = false;
      // 
      // linkLabel2
      // 
      this.linkLabel2.ActiveLinkColor = System.Drawing.Color.FromArgb(((System.Byte)(158)), ((System.Byte)(11)), ((System.Byte)(14)));
      this.linkLabel2.Font = new System.Drawing.Font("Tahoma", 16F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.linkLabel2.LinkBehavior = System.Windows.Forms.LinkBehavior.HoverUnderline;
      this.linkLabel2.LinkColor = System.Drawing.Color.FromArgb(((System.Byte)(78)), ((System.Byte)(7)), ((System.Byte)(7)));
      this.linkLabel2.Location = new System.Drawing.Point(80, 344);
      this.linkLabel2.Name = "linkLabel2";
      this.linkLabel2.Size = new System.Drawing.Size(344, 23);
      this.linkLabel2.TabIndex = 7;
      this.linkLabel2.TabStop = true;
      this.linkLabel2.Text = "http://homokaasu.org/rasterbator/gallery.gas";
      this.linkLabel2.LinkClicked += new System.Windows.Forms.LinkLabelLinkClickedEventHandler(this.LinkLabel1LinkClicked);
      // 
      // linkLabel3
      // 
      this.linkLabel3.ActiveLinkColor = System.Drawing.Color.FromArgb(((System.Byte)(158)), ((System.Byte)(11)), ((System.Byte)(14)));
      this.linkLabel3.Font = new System.Drawing.Font("Tahoma", 16F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.linkLabel3.LinkBehavior = System.Windows.Forms.LinkBehavior.HoverUnderline;
      this.linkLabel3.LinkColor = System.Drawing.Color.FromArgb(((System.Byte)(78)), ((System.Byte)(7)), ((System.Byte)(7)));
      this.linkLabel3.Location = new System.Drawing.Point(80, 320);
      this.linkLabel3.Name = "linkLabel3";
      this.linkLabel3.Size = new System.Drawing.Size(264, 23);
      this.linkLabel3.TabIndex = 22;
      this.linkLabel3.TabStop = true;
      this.linkLabel3.Text = "http://arje.net/rasterbator";
      this.linkLabel3.LinkClicked += new System.Windows.Forms.LinkLabelLinkClickedEventHandler(this.LinkLabel1LinkClicked);
      // 
      // button14
      // 
      this.button14.Font = new System.Drawing.Font("Tahoma", 15F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.button14.Location = new System.Drawing.Point(80, 336);
      this.button14.Name = "button14";
      this.button14.Size = new System.Drawing.Size(80, 32);
      this.button14.TabIndex = 27;
      this.button14.Text = "< Back";
      this.button14.Click += new System.EventHandler(this.Button14Click);
      // 
      // linkLabel1
      // 
      this.linkLabel1.ActiveLinkColor = System.Drawing.Color.FromArgb(((System.Byte)(158)), ((System.Byte)(11)), ((System.Byte)(14)));
      this.linkLabel1.Font = new System.Drawing.Font("Tahoma", 16F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.linkLabel1.LinkBehavior = System.Windows.Forms.LinkBehavior.HoverUnderline;
      this.linkLabel1.LinkColor = System.Drawing.Color.FromArgb(((System.Byte)(78)), ((System.Byte)(7)), ((System.Byte)(7)));
      this.linkLabel1.Location = new System.Drawing.Point(80, 344);
      this.linkLabel1.Name = "linkLabel1";
      this.linkLabel1.Size = new System.Drawing.Size(264, 23);
      this.linkLabel1.TabIndex = 7;
      this.linkLabel1.TabStop = true;
      this.linkLabel1.Text = "http://arje.net/rasterbator";
      this.linkLabel1.LinkClicked += new System.Windows.Forms.LinkLabelLinkClickedEventHandler(this.LinkLabel1LinkClicked);
      // 
      // button11
      // 
      this.button11.Font = new System.Drawing.Font("Tahoma", 15F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.button11.Location = new System.Drawing.Point(80, 336);
      this.button11.Name = "button11";
      this.button11.Size = new System.Drawing.Size(80, 32);
      this.button11.TabIndex = 11;
      this.button11.Text = "< Back";
      this.button11.Click += new System.EventHandler(this.Button11Click);
      // 
      // Panel7_OutputFilename
      // 
      this.Panel7_OutputFilename.Controls.Add(this.button15);
      this.Panel7_OutputFilename.Controls.Add(this.button8);
      this.Panel7_OutputFilename.Controls.Add(this.textBox2);
      this.Panel7_OutputFilename.Controls.Add(this.label9);
      this.Panel7_OutputFilename.Controls.Add(this.button9);
      this.Panel7_OutputFilename.Location = new System.Drawing.Point(0, 128);
      this.Panel7_OutputFilename.Name = "Panel7_OutputFilename";
      this.Panel7_OutputFilename.Size = new System.Drawing.Size(696, 392);
      this.Panel7_OutputFilename.TabIndex = 13;
      this.Panel7_OutputFilename.Visible = false;
      // 
      // label19
      // 
      this.label19.Font = new System.Drawing.Font("Tahoma", 16F, System.Drawing.FontStyle.Bold, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.label19.ForeColor = System.Drawing.SystemColors.ControlText;
      this.label19.Location = new System.Drawing.Point(80, 136);
      this.label19.Name = "label19";
      this.label19.Size = new System.Drawing.Size(368, 24);
      this.label19.TabIndex = 4;
      this.label19.Text = "Rasterbating, please wait...";
      // 
      // openFileDialog1
      // 
      this.openFileDialog1.Filter = "Image files (*.jpg, *.png, *.gif, *.tif, *.bmp)|*.jpg;*.gif;*.png;*.tif;*.tiff;*." +
"jpeg;*.bmp|All files (*.*)|*.*";
      this.openFileDialog1.Title = "Select Rasterbation source...";
      this.openFileDialog1.FileOk += new System.ComponentModel.CancelEventHandler(this.OpenFileDialog1FileOk);
      // 
      // label24
      // 
      this.label24.Location = new System.Drawing.Point(96, 344);
      this.label24.Name = "label24";
      this.label24.Size = new System.Drawing.Size(520, 32);
      this.label24.TabIndex = 23;
      this.label24.Text = "Check this if you want to use other programs while rasterbating. The rasterbation" +
" will take more time, but other programs become more responsive.";
      // 
      // label25
      // 
      this.label25.Font = new System.Drawing.Font("Tahoma", 11F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.label25.ForeColor = System.Drawing.Color.FromArgb(((System.Byte)(136)), ((System.Byte)(136)), ((System.Byte)(136)));
      this.label25.Location = new System.Drawing.Point(80, 376);
      this.label25.Name = "label25";
      this.label25.Size = new System.Drawing.Size(544, 24);
      this.label25.TabIndex = 8;
      this.label25.Text = "The Rasterbator Standalone 1.0 - Matias Ärje 2005 - rasterbator@arje.net - Licenc" +
"ed under GPL";
      // 
      // label22
      // 
      this.label22.Font = new System.Drawing.Font("Tahoma", 16F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.label22.ForeColor = System.Drawing.SystemColors.ControlText;
      this.label22.Location = new System.Drawing.Point(80, 216);
      this.label22.Name = "label22";
      this.label22.Size = new System.Drawing.Size(544, 48);
      this.label22.TabIndex = 21;
      this.label22.Text = "Please take a photograph of your picture and submit it to the Rasterbation Galler" +
"y! ";
      // 
      // label23
      // 
      this.label23.Location = new System.Drawing.Point(144, 176);
      this.label23.Name = "label23";
      this.label23.Size = new System.Drawing.Size(464, 16);
      this.label23.TabIndex = 21;
      this.label23.Text = "Estimated time remaining:";
      this.label23.TextAlign = System.Drawing.ContentAlignment.TopRight;
      // 
      // label20
      // 
      this.label20.Font = new System.Drawing.Font("Tahoma", 16F, System.Drawing.FontStyle.Bold, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.label20.ForeColor = System.Drawing.SystemColors.ControlText;
      this.label20.Location = new System.Drawing.Point(80, 56);
      this.label20.Name = "label20";
      this.label20.Size = new System.Drawing.Size(368, 24);
      this.label20.TabIndex = 4;
      this.label20.Text = "Rasterbation completed!";
      // 
      // label21
      // 
      this.label21.Font = new System.Drawing.Font("Tahoma", 16F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.label21.ForeColor = System.Drawing.SystemColors.ControlText;
      this.label21.Location = new System.Drawing.Point(80, 88);
      this.label21.Name = "label21";
      this.label21.Size = new System.Drawing.Size(544, 104);
      this.label21.TabIndex = 5;
      this.label21.Text = @"When printing the image in Adobe Reader, choose Page scaling: Fit to paper at the options window that comes up. If you chose horizontal paper alignment, also make sure Auto-Rotate and Center is selected. Most printers cannot print to the margins of the paper - these settings ensure that all the images will be completely printed.";
      // 
      // label18
      // 
      this.label18.Location = new System.Drawing.Point(104, 184);
      this.label18.Name = "label18";
      this.label18.Size = new System.Drawing.Size(528, 32);
      this.label18.TabIndex = 21;
      this.label18.Text = "Dot size defines the maximum size of the dots of the rasterbation. Typical range " +
"is 5-25 mm, but other sizes can be used too, if you want to play around.";
      // 
      // textBox1
      // 
      this.textBox1.Font = new System.Drawing.Font("Tahoma", 16F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.textBox1.Location = new System.Drawing.Point(80, 88);
      this.textBox1.Name = "textBox1";
      this.textBox1.Size = new System.Drawing.Size(528, 27);
      this.textBox1.TabIndex = 9;
      this.textBox1.Text = "";
      this.textBox1.TextChanged += new System.EventHandler(this.TextBox1TextChanged);
      // 
      // label15
      // 
      this.label15.Font = new System.Drawing.Font("Tahoma", 16F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.label15.ForeColor = System.Drawing.SystemColors.ControlText;
      this.label15.Location = new System.Drawing.Point(88, 152);
      this.label15.Name = "label15";
      this.label15.Size = new System.Drawing.Size(368, 24);
      this.label15.TabIndex = 13;
      this.label15.Text = "Dot size";
      this.label15.Click += new System.EventHandler(this.Label4Click);
      // 
      // label14
      // 
      this.label14.Font = new System.Drawing.Font("Tahoma", 16F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.label14.ForeColor = System.Drawing.SystemColors.ControlText;
      this.label14.Location = new System.Drawing.Point(88, 232);
      this.label14.Name = "label14";
      this.label14.Size = new System.Drawing.Size(368, 24);
      this.label14.TabIndex = 15;
      this.label14.Text = "Color mode";
      this.label14.Click += new System.EventHandler(this.Label4Click);
      // 
      // label17
      // 
      this.label17.Location = new System.Drawing.Point(104, 96);
      this.label17.Name = "label17";
      this.label17.Size = new System.Drawing.Size(520, 40);
      this.label17.TabIndex = 20;
      this.label17.Text = "This option will draw a dim rectangle around the rasterbation graphic of each pag" +
"e. The border will make it considerably easier to cut away the empty margins. If" +
" you plan not to cut out the margins, you should uncheck this.";
      // 
      // label16
      // 
      this.label16.Font = new System.Drawing.Font("Tahoma", 16F, System.Drawing.FontStyle.Bold, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.label16.ForeColor = System.Drawing.SystemColors.ControlText;
      this.label16.Location = new System.Drawing.Point(80, 32);
      this.label16.Name = "label16";
      this.label16.Size = new System.Drawing.Size(368, 24);
      this.label16.TabIndex = 8;
      this.label16.Text = "Set rasterbation options (4/5)";
      this.label16.Click += new System.EventHandler(this.Label4Click);
      // 
      // label11
      // 
      this.label11.Font = new System.Drawing.Font("Tahoma", 16F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.label11.ForeColor = System.Drawing.SystemColors.ControlText;
      this.label11.Location = new System.Drawing.Point(360, 176);
      this.label11.Name = "label11";
      this.label11.Size = new System.Drawing.Size(304, 112);
      this.label11.TabIndex = 15;
      this.label11.Click += new System.EventHandler(this.Label4Click);
      // 
      // label10
      // 
      this.label10.Font = new System.Drawing.Font("Tahoma", 16F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.label10.ForeColor = System.Drawing.SystemColors.ControlText;
      this.label10.Location = new System.Drawing.Point(240, 152);
      this.label10.Name = "label10";
      this.label10.Size = new System.Drawing.Size(368, 24);
      this.label10.TabIndex = 17;
      this.label10.Text = "mm";
      this.label10.Click += new System.EventHandler(this.Label4Click);
      // 
      // label13
      // 
      this.label13.Font = new System.Drawing.Font("Tahoma", 16F, System.Drawing.FontStyle.Bold, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.label13.ForeColor = System.Drawing.SystemColors.ControlText;
      this.label13.Location = new System.Drawing.Point(352, 64);
      this.label13.Name = "label13";
      this.label13.Size = new System.Drawing.Size(280, 24);
      this.label13.TabIndex = 8;
      this.label13.Text = "Define output size (3/5)";
      this.label13.Click += new System.EventHandler(this.Label4Click);
      // 
      // label12
      // 
      this.label12.Font = new System.Drawing.Font("Tahoma", 16F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.label12.ForeColor = System.Drawing.SystemColors.ControlText;
      this.label12.Location = new System.Drawing.Point(448, 106);
      this.label12.Name = "label12";
      this.label12.Size = new System.Drawing.Size(64, 24);
      this.label12.TabIndex = 13;
      this.label12.Text = "sheets";
      this.label12.Click += new System.EventHandler(this.Label4Click);
      // 
      // Panel2_SourceImage
      // 
      this.Panel2_SourceImage.Controls.Add(this.button11);
      this.Panel2_SourceImage.Controls.Add(this.button3);
      this.Panel2_SourceImage.Controls.Add(this.textBox1);
      this.Panel2_SourceImage.Controls.Add(this.label3);
      this.Panel2_SourceImage.Controls.Add(this.button2);
      this.Panel2_SourceImage.Location = new System.Drawing.Point(0, 128);
      this.Panel2_SourceImage.Name = "Panel2_SourceImage";
      this.Panel2_SourceImage.Size = new System.Drawing.Size(696, 392);
      this.Panel2_SourceImage.TabIndex = 9;
      this.Panel2_SourceImage.Visible = false;
      // 
      // button2
      // 
      this.button2.Enabled = false;
      this.button2.Font = new System.Drawing.Font("Tahoma", 15F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.button2.Location = new System.Drawing.Point(472, 336);
      this.button2.Name = "button2";
      this.button2.Size = new System.Drawing.Size(136, 32);
      this.button2.TabIndex = 7;
      this.button2.Text = "Continue >";
      this.button2.Click += new System.EventHandler(this.Button2Click);
      // 
      // button3
      // 
      this.button3.Font = new System.Drawing.Font("Tahoma", 15F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.button3.Location = new System.Drawing.Point(480, 128);
      this.button3.Name = "button3";
      this.button3.Size = new System.Drawing.Size(128, 28);
      this.button3.TabIndex = 10;
      this.button3.Text = "Browse...";
      this.button3.Click += new System.EventHandler(this.Button3Click);
      // 
      // label8
      // 
      this.label8.Font = new System.Drawing.Font("Tahoma", 16F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.label8.ForeColor = System.Drawing.SystemColors.ControlText;
      this.label8.Location = new System.Drawing.Point(288, 232);
      this.label8.Name = "label8";
      this.label8.Size = new System.Drawing.Size(48, 24);
      this.label8.TabIndex = 17;
      this.label8.Text = "mm";
      this.label8.Click += new System.EventHandler(this.Label4Click);
      // 
      // label9
      // 
      this.label9.Font = new System.Drawing.Font("Tahoma", 16F, System.Drawing.FontStyle.Bold, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.label9.ForeColor = System.Drawing.SystemColors.ControlText;
      this.label9.Location = new System.Drawing.Point(80, 56);
      this.label9.Name = "label9";
      this.label9.Size = new System.Drawing.Size(368, 24);
      this.label9.TabIndex = 8;
      this.label9.Text = "Save rasterbation as (5/5)";
      // 
      // Panel3_PaperSize
      // 
      this.Panel3_PaperSize.Controls.Add(this.button12);
      this.Panel3_PaperSize.Controls.Add(this.comboBox2);
      this.Panel3_PaperSize.Controls.Add(this.comboBox1);
      this.Panel3_PaperSize.Controls.Add(this.label7);
      this.Panel3_PaperSize.Controls.Add(this.label8);
      this.Panel3_PaperSize.Controls.Add(this.numericUpDown2);
      this.Panel3_PaperSize.Controls.Add(this.label6);
      this.Panel3_PaperSize.Controls.Add(this.numericUpDown1);
      this.Panel3_PaperSize.Controls.Add(this.label5);
      this.Panel3_PaperSize.Controls.Add(this.radioButton2);
      this.Panel3_PaperSize.Controls.Add(this.radioButton1);
      this.Panel3_PaperSize.Controls.Add(this.label4);
      this.Panel3_PaperSize.Controls.Add(this.button5);
      this.Panel3_PaperSize.Location = new System.Drawing.Point(0, 128);
      this.Panel3_PaperSize.Name = "Panel3_PaperSize";
      this.Panel3_PaperSize.Size = new System.Drawing.Size(696, 392);
      this.Panel3_PaperSize.TabIndex = 10;
      this.Panel3_PaperSize.Visible = false;
      this.Panel3_PaperSize.Paint += new System.Windows.Forms.PaintEventHandler(this.Panel3_PaperSizePaint);
      // 
      // label4
      // 
      this.label4.Font = new System.Drawing.Font("Tahoma", 16F, System.Drawing.FontStyle.Bold, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.label4.ForeColor = System.Drawing.SystemColors.ControlText;
      this.label4.Location = new System.Drawing.Point(80, 56);
      this.label4.Name = "label4";
      this.label4.Size = new System.Drawing.Size(368, 24);
      this.label4.TabIndex = 8;
      this.label4.Text = "Select paper size (2/5)";
      this.label4.Click += new System.EventHandler(this.Label4Click);
      // 
      // label5
      // 
      this.label5.Font = new System.Drawing.Font("Tahoma", 16F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.label5.ForeColor = System.Drawing.SystemColors.ControlText;
      this.label5.Location = new System.Drawing.Point(136, 232);
      this.label5.Name = "label5";
      this.label5.Size = new System.Drawing.Size(56, 24);
      this.label5.TabIndex = 13;
      this.label5.Text = "Width";
      this.label5.Click += new System.EventHandler(this.Label4Click);
      // 
      // label6
      // 
      this.label6.Font = new System.Drawing.Font("Tahoma", 16F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.label6.ForeColor = System.Drawing.SystemColors.ControlText;
      this.label6.Location = new System.Drawing.Point(136, 264);
      this.label6.Name = "label6";
      this.label6.Size = new System.Drawing.Size(56, 24);
      this.label6.TabIndex = 15;
      this.label6.Text = "Height";
      this.label6.Click += new System.EventHandler(this.Label4Click);
      // 
      // label7
      // 
      this.label7.Font = new System.Drawing.Font("Tahoma", 16F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.label7.ForeColor = System.Drawing.SystemColors.ControlText;
      this.label7.Location = new System.Drawing.Point(288, 264);
      this.label7.Name = "label7";
      this.label7.Size = new System.Drawing.Size(48, 24);
      this.label7.TabIndex = 18;
      this.label7.Text = "mm";
      this.label7.Click += new System.EventHandler(this.Label4Click);
      // 
      // button15
      // 
      this.button15.Font = new System.Drawing.Font("Tahoma", 15F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.button15.Location = new System.Drawing.Point(80, 336);
      this.button15.Name = "button15";
      this.button15.Size = new System.Drawing.Size(80, 32);
      this.button15.TabIndex = 28;
      this.button15.Text = "< Back";
      this.button15.Click += new System.EventHandler(this.Button15Click);
      // 
      // label1
      // 
      this.label1.Font = new System.Drawing.Font("Tahoma", 16F, System.Drawing.FontStyle.Bold, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.label1.ForeColor = System.Drawing.SystemColors.ControlText;
      this.label1.Location = new System.Drawing.Point(80, 56);
      this.label1.Name = "label1";
      this.label1.Size = new System.Drawing.Size(368, 24);
      this.label1.TabIndex = 4;
      this.label1.Text = "Welcome to The Rasterbator!";
      // 
      // label2
      // 
      this.label2.Font = new System.Drawing.Font("Tahoma", 16F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.label2.ForeColor = System.Drawing.SystemColors.ControlText;
      this.label2.Location = new System.Drawing.Point(80, 112);
      this.label2.Name = "label2";
      this.label2.Size = new System.Drawing.Size(544, 64);
      this.label2.TabIndex = 5;
      this.label2.Text = "The Rasterbator creates huge, rasterized, multi-page images from any picture. The" +
" rasterized images can be printed and assembled into extremely cool looking post" +
"ers.";
      // 
      // numericUpDown4
      // 
      this.numericUpDown4.DecimalPlaces = 1;
      this.numericUpDown4.Font = new System.Drawing.Font("Tahoma", 16F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.numericUpDown4.Location = new System.Drawing.Point(360, 104);
      this.numericUpDown4.Maximum = new System.Decimal(new int[] {
            1000,
            0,
            0,
            0});
      this.numericUpDown4.Minimum = new System.Decimal(new int[] {
            1,
            0,
            0,
            0});
      this.numericUpDown4.Name = "numericUpDown4";
      this.numericUpDown4.Size = new System.Drawing.Size(80, 27);
      this.numericUpDown4.TabIndex = 14;
      this.numericUpDown4.Value = new System.Decimal(new int[] {
            5,
            0,
            0,
            0});
      this.numericUpDown4.ValueChanged += new System.EventHandler(this.NumericUpDown4ValueChanged);
      // 
      // numericUpDown5
      // 
      this.numericUpDown5.Font = new System.Drawing.Font("Tahoma", 16F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.numericUpDown5.Location = new System.Drawing.Point(160, 152);
      this.numericUpDown5.Maximum = new System.Decimal(new int[] {
            500,
            0,
            0,
            0});
      this.numericUpDown5.Minimum = new System.Decimal(new int[] {
            1,
            0,
            0,
            0});
      this.numericUpDown5.Name = "numericUpDown5";
      this.numericUpDown5.Size = new System.Drawing.Size(72, 27);
      this.numericUpDown5.TabIndex = 14;
      this.numericUpDown5.Value = new System.Decimal(new int[] {
            10,
            0,
            0,
            0});
      // 
      // numericUpDown2
      // 
      this.numericUpDown2.Enabled = false;
      this.numericUpDown2.Font = new System.Drawing.Font("Tahoma", 16F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.numericUpDown2.Location = new System.Drawing.Point(200, 264);
      this.numericUpDown2.Maximum = new System.Decimal(new int[] {
            10000,
            0,
            0,
            0});
      this.numericUpDown2.Minimum = new System.Decimal(new int[] {
            50,
            0,
            0,
            0});
      this.numericUpDown2.Name = "numericUpDown2";
      this.numericUpDown2.Size = new System.Drawing.Size(80, 27);
      this.numericUpDown2.TabIndex = 16;
      this.numericUpDown2.Value = new System.Decimal(new int[] {
            200,
            0,
            0,
            0});
      // 
      // radioButton5
      // 
      this.radioButton5.Font = new System.Drawing.Font("Tahoma", 16F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.radioButton5.Location = new System.Drawing.Point(192, 280);
      this.radioButton5.Name = "radioButton5";
      this.radioButton5.Size = new System.Drawing.Size(248, 24);
      this.radioButton5.TabIndex = 24;
      this.radioButton5.Text = "Multi-color";
      // 
      // CustomColorDisplay
      // 
      this.CustomColorDisplay.BackColor = System.Drawing.Color.FromArgb(((System.Byte)(158)), ((System.Byte)(11)), ((System.Byte)(14)));
      this.CustomColorDisplay.Location = new System.Drawing.Point(312, 256);
      this.CustomColorDisplay.Name = "CustomColorDisplay";
      this.CustomColorDisplay.Size = new System.Drawing.Size(24, 24);
      this.CustomColorDisplay.TabIndex = 25;
      // 
      // numericUpDown1
      // 
      this.numericUpDown1.Enabled = false;
      this.numericUpDown1.Font = new System.Drawing.Font("Tahoma", 16F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.numericUpDown1.Location = new System.Drawing.Point(200, 232);
      this.numericUpDown1.Maximum = new System.Decimal(new int[] {
            10000,
            0,
            0,
            0});
      this.numericUpDown1.Minimum = new System.Decimal(new int[] {
            50,
            0,
            0,
            0});
      this.numericUpDown1.Name = "numericUpDown1";
      this.numericUpDown1.Size = new System.Drawing.Size(80, 27);
      this.numericUpDown1.TabIndex = 14;
      this.numericUpDown1.Value = new System.Decimal(new int[] {
            200,
            0,
            0,
            0});
      // 
      // radioButton1
      // 
      this.radioButton1.Checked = true;
      this.radioButton1.Font = new System.Drawing.Font("Tahoma", 16F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.radioButton1.Location = new System.Drawing.Point(88, 112);
      this.radioButton1.Name = "radioButton1";
      this.radioButton1.Size = new System.Drawing.Size(248, 24);
      this.radioButton1.TabIndex = 11;
      this.radioButton1.TabStop = true;
      this.radioButton1.Text = "Use standard paper size";
      this.radioButton1.CheckedChanged += new System.EventHandler(this.RadioButton2CheckedChanged);
      // 
      // radioButton3
      // 
      this.radioButton3.Checked = true;
      this.radioButton3.Font = new System.Drawing.Font("Tahoma", 16F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.radioButton3.Location = new System.Drawing.Point(192, 232);
      this.radioButton3.Name = "radioButton3";
      this.radioButton3.Size = new System.Drawing.Size(248, 24);
      this.radioButton3.TabIndex = 22;
      this.radioButton3.TabStop = true;
      this.radioButton3.Text = "Black";
      // 
      // radioButton2
      // 
      this.radioButton2.Font = new System.Drawing.Font("Tahoma", 16F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.radioButton2.Location = new System.Drawing.Point(88, 200);
      this.radioButton2.Name = "radioButton2";
      this.radioButton2.Size = new System.Drawing.Size(248, 24);
      this.radioButton2.TabIndex = 12;
      this.radioButton2.Text = "Use custom paper size";
      this.radioButton2.CheckedChanged += new System.EventHandler(this.RadioButton2CheckedChanged);
      // 
      // button8
      // 
      this.button8.Font = new System.Drawing.Font("Tahoma", 15F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.button8.Location = new System.Drawing.Point(480, 128);
      this.button8.Name = "button8";
      this.button8.Size = new System.Drawing.Size(128, 28);
      this.button8.TabIndex = 10;
      this.button8.Text = "Browse...";
      this.button8.Click += new System.EventHandler(this.Button8Click);
      // 
      // checkBox2
      // 
      this.checkBox2.Checked = true;
      this.checkBox2.CheckState = System.Windows.Forms.CheckState.Checked;
      this.checkBox2.Font = new System.Drawing.Font("Tahoma", 15F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.checkBox2.Location = new System.Drawing.Point(456, 288);
      this.checkBox2.Name = "checkBox2";
      this.checkBox2.Size = new System.Drawing.Size(160, 24);
      this.checkBox2.TabIndex = 20;
      this.checkBox2.Text = "Open rasticulate file";
      // 
      // colorDialog1
      // 
      this.colorDialog1.Color = System.Drawing.Color.FromArgb(((System.Byte)(158)), ((System.Byte)(11)), ((System.Byte)(14)));
      // 
      // comboBox3
      // 
      this.comboBox3.DropDownStyle = System.Windows.Forms.ComboBoxStyle.DropDownList;
      this.comboBox3.Font = new System.Drawing.Font("Tahoma", 15F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.comboBox3.Items.AddRange(new object[] {
            "wide",
            "high"});
      this.comboBox3.Location = new System.Drawing.Point(512, 104);
      this.comboBox3.Name = "comboBox3";
      this.comboBox3.Size = new System.Drawing.Size(88, 26);
      this.comboBox3.TabIndex = 22;
      this.comboBox3.SelectedIndexChanged += new System.EventHandler(this.NumericUpDown4ValueChanged);
      // 
      // comboBox2
      // 
      this.comboBox2.DropDownStyle = System.Windows.Forms.ComboBoxStyle.DropDownList;
      this.comboBox2.Font = new System.Drawing.Font("Tahoma", 15F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.comboBox2.Items.AddRange(new object[] {
            "Portrait",
            "Landscape"});
      this.comboBox2.Location = new System.Drawing.Point(368, 136);
      this.comboBox2.Name = "comboBox2";
      this.comboBox2.Size = new System.Drawing.Size(120, 26);
      this.comboBox2.TabIndex = 20;
      this.comboBox2.SelectedIndexChanged += new System.EventHandler(this.ComboBox2SelectedIndexChanged);
      // 
      // comboBox1
      // 
      this.comboBox1.DropDownStyle = System.Windows.Forms.ComboBoxStyle.DropDownList;
      this.comboBox1.Font = new System.Drawing.Font("Tahoma", 15F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.comboBox1.Items.AddRange(new object[] {
            "A4 (210x297 mm)",
            "A3 (297x420 mm)",
            "US Letter (216x279 mm)",
            "US Legal (216x356 mm)"});
      this.comboBox1.Location = new System.Drawing.Point(128, 136);
      this.comboBox1.Name = "comboBox1";
      this.comboBox1.Size = new System.Drawing.Size(224, 26);
      this.comboBox1.TabIndex = 19;
      // 
      // button6
      // 
      this.button6.Font = new System.Drawing.Font("Tahoma", 15F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.button6.Location = new System.Drawing.Point(472, 336);
      this.button6.Name = "button6";
      this.button6.Size = new System.Drawing.Size(136, 32);
      this.button6.TabIndex = 7;
      this.button6.Text = "Continue >";
      this.button6.Click += new System.EventHandler(this.Button6Click);
      // 
      // button7
      // 
      this.button7.Font = new System.Drawing.Font("Tahoma", 15F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.button7.Location = new System.Drawing.Point(344, 256);
      this.button7.Name = "button7";
      this.button7.Size = new System.Drawing.Size(80, 24);
      this.button7.TabIndex = 26;
      this.button7.Text = "Select...";
      this.button7.Click += new System.EventHandler(this.Button7Click);
      // 
      // button1
      // 
      this.button1.Font = new System.Drawing.Font("Tahoma", 15F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.button1.Location = new System.Drawing.Point(472, 336);
      this.button1.Name = "button1";
      this.button1.Size = new System.Drawing.Size(136, 32);
      this.button1.TabIndex = 6;
      this.button1.Text = "Continue >";
      this.button1.Click += new System.EventHandler(this.Button1Click);
      // 
      // radioButton4
      // 
      this.radioButton4.Font = new System.Drawing.Font("Tahoma", 16F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.radioButton4.Location = new System.Drawing.Point(192, 256);
      this.radioButton4.Name = "radioButton4";
      this.radioButton4.Size = new System.Drawing.Size(248, 24);
      this.radioButton4.TabIndex = 23;
      this.radioButton4.Text = "Custom color:";
      // 
      // button10
      // 
      this.button10.Font = new System.Drawing.Font("Tahoma", 15F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.button10.Location = new System.Drawing.Point(472, 336);
      this.button10.Name = "button10";
      this.button10.Size = new System.Drawing.Size(136, 32);
      this.button10.TabIndex = 6;
      this.button10.Text = "Close application";
      this.button10.Click += new System.EventHandler(this.Button10Click);
      // 
      // button13
      // 
      this.button13.Font = new System.Drawing.Font("Tahoma", 15F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.button13.Location = new System.Drawing.Point(360, 336);
      this.button13.Name = "button13";
      this.button13.Size = new System.Drawing.Size(80, 32);
      this.button13.TabIndex = 23;
      this.button13.Text = "< Back";
      this.button13.Click += new System.EventHandler(this.Button13Click);
      // 
      // button12
      // 
      this.button12.Font = new System.Drawing.Font("Tahoma", 15F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.button12.Location = new System.Drawing.Point(80, 336);
      this.button12.Name = "button12";
      this.button12.Size = new System.Drawing.Size(80, 32);
      this.button12.TabIndex = 21;
      this.button12.Text = "< Back";
      this.button12.Click += new System.EventHandler(this.Button12Click);
      // 
      // progressBar1
      // 
      this.progressBar1.Location = new System.Drawing.Point(80, 160);
      this.progressBar1.Name = "progressBar1";
      this.progressBar1.Size = new System.Drawing.Size(528, 16);
      this.progressBar1.TabIndex = 8;
      // 
      // Panel1_Welcome
      // 
      this.Panel1_Welcome.BackColor = System.Drawing.Color.White;
      this.Panel1_Welcome.Controls.Add(this.label25);
      this.Panel1_Welcome.Controls.Add(this.button1);
      this.Panel1_Welcome.Controls.Add(this.linkLabel1);
      this.Panel1_Welcome.Controls.Add(this.label1);
      this.Panel1_Welcome.Controls.Add(this.label2);
      this.Panel1_Welcome.Location = new System.Drawing.Point(0, 128);
      this.Panel1_Welcome.Name = "Panel1_Welcome";
      this.Panel1_Welcome.Size = new System.Drawing.Size(696, 392);
      this.Panel1_Welcome.TabIndex = 5;
      // 
      // Panel4_OutputSize
      // 
      this.Panel4_OutputSize.Controls.Add(this.button13);
      this.Panel4_OutputSize.Controls.Add(this.comboBox3);
      this.Panel4_OutputSize.Controls.Add(this.pictureBox2);
      this.Panel4_OutputSize.Controls.Add(this.label11);
      this.Panel4_OutputSize.Controls.Add(this.numericUpDown4);
      this.Panel4_OutputSize.Controls.Add(this.label12);
      this.Panel4_OutputSize.Controls.Add(this.label13);
      this.Panel4_OutputSize.Controls.Add(this.button4);
      this.Panel4_OutputSize.Location = new System.Drawing.Point(0, 128);
      this.Panel4_OutputSize.Name = "Panel4_OutputSize";
      this.Panel4_OutputSize.Size = new System.Drawing.Size(696, 392);
      this.Panel4_OutputSize.TabIndex = 11;
      this.Panel4_OutputSize.Visible = false;
      // 
      // textBox2
      // 
      this.textBox2.Font = new System.Drawing.Font("Tahoma", 16F, System.Drawing.FontStyle.Regular, System.Drawing.GraphicsUnit.World, ((System.Byte)(0)));
      this.textBox2.Location = new System.Drawing.Point(80, 88);
      this.textBox2.Name = "textBox2";
      this.textBox2.Size = new System.Drawing.Size(528, 27);
      this.textBox2.TabIndex = 9;
      this.textBox2.Text = "";
      // 
      // MainForm
      // 
      this.AutoScaleBaseSize = new System.Drawing.Size(5, 13);
      this.BackColor = System.Drawing.Color.White;
      this.ClientSize = new System.Drawing.Size(692, 523);
      this.Controls.Add(this.Panel5_Options);
      this.Controls.Add(this.Panel7_OutputFilename);
      this.Controls.Add(this.Panel4_OutputSize);
      this.Controls.Add(this.Panel3_PaperSize);
      this.Controls.Add(this.Panel1_Welcome);
      this.Controls.Add(this.Panel2_SourceImage);
      this.Controls.Add(this.Panel9_Completed);
      this.Controls.Add(this.Panel8_Rasterbating);
      this.Controls.Add(this.pictureBox1);
      this.Icon = ((System.Drawing.Icon)(resources.GetObject("$this.Icon")));
      this.MaximumSize = new System.Drawing.Size(700, 550);
      this.MinimumSize = new System.Drawing.Size(700, 550);
      this.Name = "MainForm";
      this.Text = "The Rasterbator";
      this.Load += new System.EventHandler(this.MainFormLoad);
      this.Panel8_Rasterbating.ResumeLayout(false);
      this.Panel9_Completed.ResumeLayout(false);
      this.Panel5_Options.ResumeLayout(false);
      this.Panel7_OutputFilename.ResumeLayout(false);
      this.Panel2_SourceImage.ResumeLayout(false);
      this.Panel3_PaperSize.ResumeLayout(false);
      ((System.ComponentModel.ISupportInitialize)(this.numericUpDown4)).EndInit();
      ((System.ComponentModel.ISupportInitialize)(this.numericUpDown5)).EndInit();
      ((System.ComponentModel.ISupportInitialize)(this.numericUpDown2)).EndInit();
      ((System.ComponentModel.ISupportInitialize)(this.numericUpDown1)).EndInit();
      this.Panel1_Welcome.ResumeLayout(false);
      this.Panel4_OutputSize.ResumeLayout(false);
      this.ResumeLayout(false);
    }
		#endregion

		void Button1Click(object sender, System.EventArgs e)
		{
		  Panel1_Welcome.Visible=false;
		  Panel2_SourceImage.Visible=true;
		}
		
		void Button2Click(object sender, System.EventArgs e)
		{
		  Panel2_SourceImage.Visible=false;
		  Panel3_PaperSize.Visible=true;
		  if(comboBox1.SelectedIndex<0) comboBox1.SelectedIndex=0;
		  if(comboBox2.SelectedIndex<0) comboBox2.SelectedIndex=0;
		}
		
		void Button3Click(object sender, System.EventArgs e)
		{
		  openFileDialog1.ShowDialog();
		}
		
		void OpenFileDialog1FileOk(object sender, System.ComponentModel.CancelEventArgs e)
		{
		  textBox1.Text=openFileDialog1.FileName;
		}
		
		void TextBox1TextChanged(object sender, System.EventArgs e)
		{
		  button2.Enabled=File.Exists(textBox1.Text);
		  string s=textBox1.Text;
		  if(s.IndexOf(".")<0) s+=".";
		  s=s.Substring(0, s.LastIndexOf("."));
		  string fn=s;
		  int c=0;
		  while(File.Exists(fn+".pdf")) {
		    c++;
		    fn=s+"_"+c;
		  }
		  textBox2.Text=fn+".pdf";
		}
		
		void Label4Click(object sender, System.EventArgs e)
		{
		  
		}
		
		void ComboBox2SelectedIndexChanged(object sender, System.EventArgs e) {
		  
		}
		
		Bitmap DisplayImage=null;
		Bitmap OriginalImage=null;

		// in millimeters
		decimal PaperWidth=210M;
		decimal PaperHeight=297M;
				  
		void Button5Click(object sender, System.EventArgs e) {
		  Panel3_PaperSize.Visible=false;
		  Panel4_OutputSize.Visible=true;
		  if(comboBox3.SelectedIndex<0) comboBox3.SelectedIndex=0;
		  
		  if(radioButton1.Checked) {
		    decimal s1=0, s2=0;
		    switch(comboBox1.SelectedIndex) {
		      case 0 : s1=210M; s2=297M; break;
		      case 1 : s1=297M; s2=420M; break;
		      case 2 : s1=216M; s2=279M; break;
		      case 3 : s1=216M; s2=356M; break;
		    }
		    
		    if(comboBox2.SelectedIndex==0) {
		      PaperWidth=s1;
		      PaperHeight=s2;
		    } else {
		      PaperWidth=s2;
		      PaperHeight=s1;
		    }		    
		    
		  } else {
		    PaperWidth=numericUpDown1.Value;
		    PaperHeight=numericUpDown2.Value;
		  }
		  
		  OriginalImage=Image.FromFile(textBox1.Text) as Bitmap;
		  double w,h;
		  if(OriginalImage.Width>OriginalImage.Height) {
        w=300;
        h=300*(double)OriginalImage.Height/(double)OriginalImage.Width;
		  } else {
        h=300;
        w=300*(double)OriginalImage.Width/(double)OriginalImage.Height;
		  }
		  DisplayImage=new Bitmap(OriginalImage, new Size((int)w, (int)h));
		  
     
      DisplayPaging();
		}
		
		decimal OutputWidth=0, OutputHeight=0;
		
		void DisplayPaging() {
		  if(DisplayImage==null) return;
		  
		  decimal w=DisplayImage.Width;
		  decimal h=DisplayImage.Height;
		  Bitmap B=new Bitmap(DisplayImage);
		  Graphics G=Graphics.FromImage(B);
		  
		  decimal Pages=numericUpDown4.Value;
		  
		  bool AdjustWidth=true;
		  if(comboBox3.SelectedIndex==1) AdjustWidth=false;
		  
		  decimal DisplayPageWidth=1, DisplayPageHeight=1, PagesX=1, PagesY=1;
		  
		  if(AdjustWidth) {
		    DisplayPageWidth=w/Pages;
		    DisplayPageHeight=PaperHeight/PaperWidth*DisplayPageWidth;
		    PagesX=Pages;
		    PagesY=PaperWidth/PaperHeight*PagesX*h/w;
		  } else {
		    DisplayPageHeight=h/Pages;
		    DisplayPageWidth=PaperWidth/PaperHeight*DisplayPageHeight;
		    PagesY=Pages;
		    PagesX=PaperHeight/PaperWidth*PagesY*w/h;
		  }
		  
		  Pen DarkPen=new Pen(Color.FromArgb(0x88, 0, 0, 0));
		  Pen LightPen=new Pen(Color.FromArgb(0x88, 255, 255, 255));
		  
		  decimal p=DisplayPageWidth;
		  while(p<w) {
		    int x=(int)Math.Round(p);
		    G.DrawLine(LightPen, x, 0, x, (int)h);
		    G.DrawLine(DarkPen, x+1, 0, x+1, (int)h);
		    p+=DisplayPageWidth;		     		    
		  }

		  p=DisplayPageHeight;
		  while(p<h) {
		    int y=(int)Math.Round(p);
		    G.DrawLine(LightPen, 0, y, (int)w, y);
		    G.DrawLine(DarkPen, 0, y+1, (int)w, y+1);
		    p+=DisplayPageHeight;		     		    
		  }
		  
		  G.Save();
		  G.Dispose();
		  
		  Bitmap B2=new Bitmap(300, 300);
		  
      G=Graphics.FromImage(B2);
      G.FillRectangle(new SolidBrush(Color.White), 0, 0, 300, 300);
      
      TextureBrush TB=new TextureBrush(B, WrapMode.Clamp);
      TB.TranslateTransform((int)Math.Round((300-(decimal)w)/2), (int)Math.Round(((decimal)300-h)/2));
    
      G.FillRectangle(TB, (int)Math.Round(((decimal)300-w)/2), (int)Math.Round(((decimal)300-h)/2), (int)w, (int)h);
      G.Save();

      pictureBox2.Image=B2 as Image;
      
      int px=(int)Math.Ceiling((double)PagesX);
      int py=(int)Math.Ceiling((double)PagesY);
      
      string Stats="";
      
      OutputWidth=PaperWidth*PagesX;
      OutputHeight=PaperHeight*PagesY;
      
      if(OutputWidth>1000 || OutputHeight>1000) 
        Stats="Output image size:\n"+(OutputWidth/1000).ToString("f2")+" x "+(OutputHeight/1000).ToString("f2")+" m\n\n";
      else 
        Stats="Output image size:\n"+(OutputWidth/10).ToString("f1")+" x "+(OutputHeight/10).ToString("f1")+" cm\n\n";
        
      Stats+="Paper consumption:\n"+px+" x "+py+" = "+(px*py)+" sheet"+(px*py==1 ? "" : "s");
      
      label11.Text=Stats;
		}
		
		void NumericUpDown4ValueChanged(object sender, System.EventArgs e)
		{
		  DisplayPaging();
		}
		
		void Panel5_OptionsPaint(object sender, System.Windows.Forms.PaintEventArgs e)
		{
		  
		}
		
		void Button7Click(object sender, System.EventArgs e)
		{
		  if(colorDialog1.ShowDialog()!=DialogResult.OK) return;
      radioButton4.Checked=true;
 	    CustomColorDisplay.BackColor=colorDialog1.Color;
		}
		
		void Button4Click(object sender, System.EventArgs e)
		{
		  DisplayPaging();
		  Panel4_OutputSize.Visible=false;
		  Panel5_Options.Visible=true;
		}
		
		void Panel3_PaperSizePaint(object sender, System.Windows.Forms.PaintEventArgs e)
		{
		  
		}
		
		void MainFormLoad(object sender, System.EventArgs e)
		{
		}
		
		void RadioButton2CheckedChanged(object sender, System.EventArgs e)
		{
		  comboBox1.Enabled=radioButton1.Checked;
		  comboBox2.Enabled=radioButton1.Checked;
		  numericUpDown1.Enabled=radioButton2.Checked;
		  numericUpDown2.Enabled=radioButton2.Checked;
		}
		
		void NumericUpDown4KeyUp(object sender, System.Windows.Forms.KeyEventArgs e)
		{
		  DisplayPaging();
		}
		
		void Button8Click(object sender, System.EventArgs e)
		{
		  if(saveFileDialog1.ShowDialog()!=DialogResult.OK) return;
		  textBox2.Text=saveFileDialog1.FileName;
		}
		
		void Button6Click(object sender, System.EventArgs e)
		{
		  if(numericUpDown5.Value<5M) {
		    if(MessageBox.Show("Rasterbating using small dot sizes (less than 5 mm) consumes lots of memory and takes a long time. Do you want to continue?",
		                       "Confirm options",
		                       MessageBoxButtons.YesNo)==DialogResult.No) return;
		  }
		  Panel5_Options.Visible=false;
		  Panel7_OutputFilename.Visible=true;
		}
		
		void LinkLabel1LinkClicked(object sender, System.Windows.Forms.LinkLabelLinkClickedEventArgs e)
		{
		  System.Diagnostics.Process.Start("http://arje.net/rasterbator");
		}
		
		Rasterbator R;
		
		void Button9Click(object sender, System.EventArgs e)
		{
		  Panel7_OutputFilename.Visible=false;
		  Panel8_Rasterbating.Visible=true;
		  
		  NameValueCollection S=new NameValueCollection();
		  		  
		  S["ImageWidth"]=((int)OutputWidth).ToString();
		  S["ImageHeight"]=((int)OutputHeight).ToString();
		  S["PaperWidth"]=PaperWidth.ToString();
		  S["PaperHeight"]=PaperHeight.ToString();
		  S["RasterSize"]=numericUpDown5.Value.ToString();
		  S["CroppingRectangle"]=checkBox1.Checked.ToString();
		  
		  if(radioButton3.Checked) {
		    S["RasterColor"]="000000";
		  }
		  if(radioButton4.Checked) {
	  	  Color C=colorDialog1.Color;
	  	  S["RasterColor"]=C.R.ToString("x2")+C.G.ToString("x2")+C.B.ToString("x2");
		  }
		  if(radioButton5.Checked) {
		    S["RasterColor"]="avg";
		  }

		  S["OriginalImageWidth"]=OriginalImage.Width.ToString();
		  S["OriginalImageHeight"]=OriginalImage.Height.ToString();
		  S["DisplayImageWidth"]=OriginalImage.Width.ToString();
		  S["DisplayImageHeight"]=OriginalImage.Height.ToString();
		  S["x1"]="0";
		  S["y1"]="0";
		  S["x2"]=OriginalImage.Width.ToString();
		  S["y2"]=OriginalImage.Height.ToString();
		  S["OriginalFilename"]=textBox1.Text;
		  S["TargetFilename"]=textBox2.Text;
		  S["WorkDirectory"]=textBox2.Text.Substring(0, textBox2.Text.LastIndexOf("\\")+1);
		  
		  R=new Rasterbator(S);
      R.Worker=new Thread(new ThreadStart(R.CreateImage));
      R.Worker.Priority=ThreadPriority.Lowest;
      R.Worker.Start();
      
      label23.Text="";
      StartTime=DateTime.Now;
      
		  progressBar1.ForeColor=Color.FromArgb(0xff, 0x4e, 0x07, 0x07);
      timer1.Enabled=true;
		}
		
		DateTime StartTime;
		double LastProgress=-1;
		
		void Timer1Tick(object sender, System.EventArgs e)
		{
		  double p=R.Progress;
		  progressBar1.Value=(int)(p*100d);
		  
		  if(StartTime.AddSeconds(2)<DateTime.Now && p!=LastProgress) {
		    
		    double SecondsElapsed=DateTime.Now.Subtract(StartTime).TotalSeconds;
		    int SecondsRemaining=(int)(SecondsElapsed/p*(1d-p));
		    
		    if(SecondsRemaining>0) {
  		    string s="Estimated time remaining:";
  		    if(SecondsRemaining>0) {
  		      int m=SecondsRemaining/60;
  		      if(m>0) s+=" "+m+" minute"+(m==1 ? "" : "s");
  		      SecondsRemaining%=60;
  		    }
  		    s+=" "+SecondsRemaining+" second"+(SecondsRemaining==1 ? "" : "s");
		    
  		    label23.Text=s;
		    } else {
  		    label23.Text="";
		    }
		    
		    LastProgress=p;
		  }
		     
		  if(R.Worker.ThreadState==ThreadState.Running) return;
		  OriginalImage.Dispose();
		  Panel8_Rasterbating.Visible=false;
		  Panel9_Completed.Visible=true;
		}
		
		void Button10Click(object sender, System.EventArgs e)
		{
		  if(checkBox2.Checked) if(File.Exists(textBox2.Text)) System.Diagnostics.Process.Start(textBox2.Text);
		  Close();
		}
		
		void LowPriorityRasterbationCheckedChanged(object sender, System.EventArgs e)
		{
		  R.Worker.Priority=LowPriorityRasterbation.Checked ? ThreadPriority.BelowNormal : ThreadPriority.Normal;
		}
		
		void Button11Click(object sender, System.EventArgs e)
		{
		  Panel2_SourceImage.Visible=false;
		  Panel1_Welcome.Visible=true;
		}
		
		void Button12Click(object sender, System.EventArgs e)
		{
		  Panel3_PaperSize.Visible=false;
		  Panel2_SourceImage.Visible=true;
		}
		
		void Button13Click(object sender, System.EventArgs e)
		{
		  Panel4_OutputSize.Visible=false;
		  Panel3_PaperSize.Visible=true;
		}
		
		void Button14Click(object sender, System.EventArgs e)
		{
		  Panel5_Options.Visible=false;
		  Panel4_OutputSize.Visible=true;
		  Button5Click(sender, e);
		}
		
		void Button15Click(object sender, System.EventArgs e)
		{
		  Panel7_OutputFilename.Visible=false;
		  Panel5_Options.Visible=true;
		}
		
	}
}
